//! Executor for the bash tool — handles all subprocess spawning, I/O capture, and timeout logic.
//!
//! This module contains the core logic for validating commands, spawning subprocesses,
//! capturing merged stdout+stderr, handling timeouts, and truncating output.
//! All subprocess I/O is async via tokio::process.

use crate::args::BashArgs;
use crate::output::BashOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Maximum output characters returned to the model (stdout + stderr merged).
const MAX_OUTPUT_CHARS: usize = 10_000;

/// Executes the bash tool with the given arguments.
///
/// Spawns a stateless subprocess (`sh -c` on Unix, `cmd /C` on Windows), captures
/// merged stdout+stderr, handles optional timeout, and returns the exit code and output.
/// Each call is independent — no state persists between calls.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized bash arguments containing the command and optional timeout.
///
/// # Returns
/// A `ToolResult` with either success (JSON BashOutput) or failure (Text error message).
pub async fn execute(call_id: ToolCallId, args: BashArgs) -> ToolResult {
    // Step 1: Validate command is non-empty.
    // An empty command is a no-op and indicates a mistake by the model.
    if args.command.trim().is_empty() {
        return ToolResult {
            call_id,
            name: "bash".to_string(),
            content: ToolContent::Text("command is empty".to_string()),
            is_error: true,
        };
    }

    // Step 2: Spawn the process.
    // Use `sh -c` on Unix, `cmd /C` on Windows. Pipe both stdout and stderr
    // so we can capture and merge them.
    let mut child = match Command::new(if cfg!(windows) { "cmd" } else { "sh" })
        .args(if cfg!(windows) {
            vec!["/C", &args.command]
        } else {
            vec!["-c", &args.command]
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "bash".to_string(),
                content: ToolContent::Text(format!("failed to spawn process: {}", e)),
                is_error: true,
            };
        }
    };

    // Step 3: Take stdout and stderr handles before waiting.
    // These are guaranteed to exist because we set Stdio::piped() above.
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");

    // Step 4: Read both streams concurrently and wait for exit.
    // Use tokio::join! to read stdout and stderr in parallel — prevents deadlock
    // when one stream fills its buffer while we're reading the other.
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

    // Step 5: Apply timeout wrapper around the read+wait operations.
    // If timeout_ms is provided, wrap in tokio::time::timeout. Otherwise, await directly.
    let (timed_out, stdout_bytes, stderr_bytes, exit_code) =
        if let Some(ms) = args.timeout_ms {
            match tokio::time::timeout(
                std::time::Duration::from_millis(ms),
                async {
                    let (out, err, status) = tokio::join!(read_stdout, read_stderr, wait);
                    (out, err, status)
                },
            )
            .await
            {
                Ok((out, err, status)) => {
                    // Command completed within timeout.
                    let code = status
                        .map(|s| s.code().unwrap_or(-1))
                        .unwrap_or(-1);
                    (false, out, err, code)
                }
                Err(_timeout) => {
                    // Timeout occurred — kill the process.
                    let _ = child.kill().await;
                    // Drain whatever partial output was buffered.
                    let mut out = Vec::new();
                    let mut err = Vec::new();
                    let _ = stdout.read_to_end(&mut out).await;
                    let _ = stderr.read_to_end(&mut err).await;
                    (true, out, err, -1i32)
                }
            }
        } else {
            // No timeout — wait indefinitely for the command to complete.
            let (out, err, status) = tokio::join!(read_stdout, read_stderr, wait);
            let code = status
                .map(|s| s.code().unwrap_or(-1))
                .unwrap_or(-1);
            (false, out, err, code)
        };

    // Step 6: Merge stdout + stderr and truncate.
    // Merge stdout first, then stderr (same order as `2>&1` in shells).
    let mut merged = String::new();

    // Append stdout
    if !stdout_bytes.is_empty() {
        merged.push_str(&String::from_utf8_lossy(&stdout_bytes));
    }
    // Append stderr
    if !stderr_bytes.is_empty() {
        merged.push_str(&String::from_utf8_lossy(&stderr_bytes));
    }

    // Check if truncation is needed and truncate at a char boundary (not byte boundary).
    let truncated = merged.chars().count() > MAX_OUTPUT_CHARS;
    if truncated {
        let truncated_output: String = merged.chars().take(MAX_OUTPUT_CHARS).collect();
        merged = truncated_output;
    }

    // Step 7: Return success result.
    // Construct the output with the command, exit code, output, and metadata.
    let output = BashOutput {
        command: args.command.clone(),
        exit_code,
        output: merged,
        truncated,
        timed_out,
    };

    ToolResult {
        call_id,
        name: "bash".to_string(),
        content: ToolContent::Json(serde_json::to_value(&output).unwrap_or_else(|e| {
            serde_json::json!({ "error": format!("serialization bug: {}", e) })
        })),
        is_error: false,
    }
}
