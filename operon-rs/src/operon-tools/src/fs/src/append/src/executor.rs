//! Executor for the append tool — handles all file I/O via O_APPEND mode.
//!
//! This module contains the core logic for validating paths, checking file existence,
//! and atomically appending content to the end of a file using append mode.
//! All file I/O is async via tokio::fs.
//!
//! ## Output format (plain text, ToolContent::Text)
//!
//! Success:    "{path} done"
//! Any error:  "{path}\nERROR: {reason}"
//!
//! is_error is always false — the model reads the inline ERROR text.

use crate::args::AppendArgs;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use tokio::io::AsyncWriteExt;

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Builds a plain-text ToolResult that signals an error to the model.
///
/// The format is "{path}\nERROR: {reason}" so the model can immediately identify
/// which file failed and why. is_error is intentionally false — the model reads
/// the inline ERROR text rather than relying on the is_error flag.
fn error_result(call_id: ToolCallId, path: &str, reason: &str) -> ToolResult {
    ToolResult {
        call_id,
        name: "append".to_string(),
        content: ToolContent::Text(format!("{}\nERROR: {}", path, reason)),
        is_error: false,
        read_paths: None,
    }
}

// ── Executor ───────────────────────────────────────────────────────────────────

/// Executes the append tool with the given arguments.
///
/// 1. Rejects empty content with an inline ERROR (so the model sees it inline).
/// 2. Checks that the target path exists and is not a directory.
/// 3. Opens the file with O_APPEND and writes the content to the end.
/// 4. Returns a plain-text ToolResult (success or error).
///
/// The append operation is atomic for writes under the pipe-buffer size per POSIX,
/// and safe for all practical agent use cases. No temp file is needed.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`:    The parsed append arguments containing the path and content.
///
/// # Returns
/// A `ToolResult` with ToolContent::Text — never Json. is_error is always false.
pub async fn execute(call_id: ToolCallId, args: AppendArgs) -> ToolResult {
    // Step 1: Reject empty content immediately.
    // Appending empty content is a no-op and indicates a model mistake.
    // The error is returned inline (not as ArgsParse) so the model sees it in output.
    if args.content.is_empty() {
        return error_result(call_id, &args.path, "content is empty");
    }

    let path = std::path::Path::new(&args.path);

    // Step 2: Check that the file exists and is not a directory.
    // We need to verify the file exists before attempting to append, and ensure
    // it's a regular file (or symlink), not a directory.
    match tokio::fs::metadata(path).await {
        Ok(meta) => {
            // File exists — reject if it's a directory.
            if meta.is_dir() {
                return error_result(call_id, &args.path, "path is a directory");
            }
            // is_file() or is_symlink() — proceed to append.
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File does not exist — tell the model to use write instead.
            return error_result(call_id, &args.path, "file does not exist");
        }
        Err(e) => {
            // Some other I/O error (permission denied, etc.)
            return error_result(call_id, &args.path, &format!("{}", e));
        }
    }

    // Step 3: Open in append mode and write the content.
    // tokio::fs::OpenOptions with .append(true) sets O_APPEND at the OS level,
    // which positions the write cursor atomically at EOF. No temp file needed.
    let mut file = match tokio::fs::OpenOptions::new().append(true).open(path).await {
        Ok(f) => f,
        Err(e) => {
            return error_result(call_id, &args.path, &format!("{}", e));
        }
    };

    // Write all bytes from the content string.
    if let Err(e) = file.write_all(args.content.as_bytes()).await {
        return error_result(call_id, &args.path, &format!("{}", e));
    }

    // Flush to ensure all bytes reach the OS before we return success.
    if let Err(e) = file.flush().await {
        return error_result(call_id, &args.path, &format!("{}", e));
    }

    // Step 4: Return plain-text success.
    ToolResult {
        call_id,
        name: "append".to_string(),
        content: ToolContent::Text(format!("{} done", args.path)),
        is_error: false,
        read_paths: None,
    }
}
