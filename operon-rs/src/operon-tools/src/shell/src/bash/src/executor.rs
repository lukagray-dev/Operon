// executor.rs — Subprocess spawning, I/O capture, and timeout logic for the bash tool.
//
// Responsibilities:
//   1. Validate `path` is an existing directory on disk.
//   2. Spawn the subprocess with the correct shell and working directory.
//   3. Capture merged stdout + stderr concurrently (prevents pipe buffer deadlock).
//   4. Apply the always-present timeout, killing the process if it expires.
//   5. Truncate the merged output to MAX_OUTPUT_CHARS.
//   6. Return a plain-text ToolResult with exit code, merged output, and status markers.
//
// NOTE: This module never returns SessionError or any session-level type.
// It only knows about ToolCallId, ToolResult, ToolContent, and BashArgs.
// All policy decisions are made upstream by operon-policy before this runs.

use crate::args::BashArgs;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

// Maximum number of characters returned to the model (stdout + stderr merged).
// Keeps context window consumption predictable for the caller.
const MAX_OUTPUT_CHARS: usize = 10_000;

// ─────────────────────────────────────────────────────────────────────────────
// execute
// ─────────────────────────────────────────────────────────────────────────────

/// Executes the bash tool with the given arguments.
///
/// Spawns a stateless subprocess (`sh -c` on Unix, `cmd /C` on Windows)
/// in the specified `path`, captures merged stdout+stderr, applies the
/// timeout, and returns the result as plain text.
///
/// # Output format
///
/// The first line is always `exit: {N}`. This is followed by the merged
/// stdout+stderr. Suffix lines are appended when applicable:
///   - `***truncated***`  — output was capped at 10,000 chars
///   - `***timed out***`  — the process was killed by the timeout
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The parsed bash arguments — path, command, and timeout_ms.
///
/// # Returns
/// Always returns a `ToolResult`. Failures are encoded as
/// `ToolResult { is_error: true, content: ToolContent::Text(reason) }`.
/// This function never panics.
pub async fn execute(call_id: ToolCallId, args: BashArgs) -> ToolResult {
    // ── Step 1: Validate path exists and is a directory ────────────────────────
    // The policy layer already confirmed path is within an allowed directory,
    // but we still check existence here as defence-in-depth. The policy check
    // is a permission decision; this is a runtime reality check.
    let cwd_path = Path::new(&args.path);

    if !cwd_path.is_absolute() {
        return make_error(
            call_id,
            &format!("path must be an absolute path, got: {:?}", args.path),
        );
    }

    if !cwd_path.exists() {
        return make_error(call_id, &format!("path does not exist: {:?}", args.path));
    }

    if !cwd_path.is_dir() {
        return make_error(call_id, &format!("path is not a directory: {:?}", args.path));
    }

    // ── Step 2: Spawn the subprocess ───────────────────────────────────────────
    // Use `sh -c` on Unix, `cmd /C` on Windows.
    // Pipe both stdout and stderr so we can capture and merge them.
    // `.current_dir(cwd_path)` sets the working directory for the subprocess.
    let mut child = match Command::new(if cfg!(windows) { "cmd" } else { "sh" })
        .args(if cfg!(windows) {
            vec!["/C", &args.command]
        } else {
            vec!["-c", &args.command]
        })
        .current_dir(cwd_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return make_error(call_id, &format!("failed to spawn process: {}", e));
        }
    };

    // ── Step 3: Take I/O handles before waiting ────────────────────────────────
    // Must be taken before `wait()` is called; otherwise the handles are consumed.
    // These are guaranteed present because we set Stdio::piped() above.
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");

    // ── Step 4: Read both streams concurrently and wait for exit ───────────────
    // Use tokio::join! to drive stdout read, stderr read, and wait() concurrently.
    // This prevents the classic deadlock: if one pipe's buffer fills up while
    // we're blocked reading the other, the subprocess stalls and we deadlock.
    let read_stdout = async {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf).await;
        buf
    };
    let read_stderr = async {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf).await;
        buf
    };
    let wait = child.wait();

    // ── Step 5: Apply always-present timeout wrapper ───────────────────────────
    // `timeout_ms` is always set (defaults to 30 minutes if not provided by the model).
    // On timeout: kill the process, drain whatever partial output was buffered,
    // and set timed_out=true with exit_code=-1.
    let (timed_out, stdout_bytes, stderr_bytes, exit_code) = match tokio::time::timeout(
        std::time::Duration::from_millis(args.timeout_ms),
        async { tokio::join!(read_stdout, read_stderr, wait) },
    )
    .await
    {
        Ok((out, err, status)) => {
            // Command finished within the timeout window. Resolve the exit code.
            let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
            (false, out, err, code)
        }
        Err(_timeout) => {
            // Timeout expired — kill the subprocess gracefully and drain buffers.
            let _ = child.kill().await;
            // Drain whatever partial output was buffered before the kill signal.
            let mut out = Vec::new();
            let mut err = Vec::new();
            let _ = stdout.read_to_end(&mut out).await;
            let _ = stderr.read_to_end(&mut err).await;
            (true, out, err, -1i32)
        }
    };

    // ── Step 6: Merge stdout + stderr and truncate ─────────────────────────────
    // Stdout first, then stderr — same order as `2>&1` shell redirection.
    // Truncate at MAX_OUTPUT_CHARS char boundaries (not byte boundaries) to
    // avoid producing invalid UTF-8 output.
    let mut merged = String::new();

    if !stdout_bytes.is_empty() {
        merged.push_str(&String::from_utf8_lossy(&stdout_bytes));
    }
    if !stderr_bytes.is_empty() {
        merged.push_str(&String::from_utf8_lossy(&stderr_bytes));
    }

    let truncated = merged.chars().count() > MAX_OUTPUT_CHARS;
    if truncated {
        // Take exactly MAX_OUTPUT_CHARS characters — safe at any Unicode boundary.
        merged = merged.chars().take(MAX_OUTPUT_CHARS).collect();
    }

    // ── Step 7: Return plain-text success result ───────────────────────────────
    // Format: first line is `exit: {N}`, then merged output, then optional
    // status markers (`***truncated***`, `***timed out***`).
    //
    // Note: non-zero exit codes and timeouts are NOT tool errors — they are
    // normal outcomes the model receives and decides how to handle.
    // Only process spawn failures and validation errors are `is_error: true`.
    let mut output_parts = vec![format!("exit: {}", exit_code), merged];

    if truncated {
        output_parts.push("***truncated***".to_string());
    }
    if timed_out {
        output_parts.push("***timed out***".to_string());
    }

    let output_text = output_parts.join("\n");

    ToolResult {
        call_id,
        name: "bash".to_string(),
        content: ToolContent::Text(output_text),
        is_error: false,
        read_paths: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Constructs a `ToolResult` representing a tool-level error (is_error = true).
///
/// Used for validation failures (bad path, spawn error).
/// The `reason` string is returned to the model verbatim so it can recover.
fn make_error(call_id: ToolCallId, reason: &str) -> ToolResult {
    ToolResult {
        call_id,
        name: "bash".to_string(),
        content: ToolContent::Text(reason.to_string()),
        is_error: true,
        // read_paths is None for bash errors — bash never reads files through the read tool.
        read_paths: None,
    }
}
