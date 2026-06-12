//! Executor for the delete tool — handles all file I/O and deletion operations.
//!
//! This module contains the core logic for validating paths, checking file/directory existence,
//! and performing either trash or permanent deletion. All file I/O is async via tokio::fs,
//! and blocking operations (trash, remove_dir_all) are executed in spawn_blocking to avoid
//! blocking the async runtime.
//!
//! # Output format (plain text)
//!
//! Success:
//!   "{path} permanently deleted (file|dir)"
//!   "{path} moved to trash (file|dir)"
//!
//! Failure (inline, is_error: false):
//!   "path does not exist: {path}"
//!   "failed to access path: {path}: {e}"
//!   "failed to move to trash: {e}"
//!   "failed to delete file: {e}"
//!   "failed to delete directory: {e}"
//!   "internal error: delete task panicked"

use crate::args::DeleteArgs;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};

/// Executes the delete tool with the given arguments.
///
/// Validates that the path exists, determines whether it's a file or directory,
/// then either moves it to trash (default, recoverable) or permanently deletes it.
/// Blocking operations run inside spawn_blocking to avoid stalling the async runtime.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args`: The parsed delete arguments containing the path and permanent flag.
///
/// # Returns
/// A `ToolResult` with `is_error: false` always. Errors are embedded inline.
pub async fn execute(call_id: ToolCallId, args: DeleteArgs) -> ToolResult {
    // Step 1: Verify the path exists and determine file/dir kind.
    let path = std::path::Path::new(&args.path);

    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Path does not exist — report inline (is_error: false per spec).
            return ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text(format!("path does not exist: {}", args.path)),
                is_error: false,
                read_paths: None,
            };
        }
        Err(e) => {
            return ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text(format!(
                    "failed to access path: {}: {}",
                    args.path, e
                )),
                is_error: false,
                read_paths: None,
            };
        }
    };

    // Determine whether the target is a file or directory for the output label.
    let kind_str = if meta.is_dir() { "dir" } else { "file" };

    // Step 2: Execute deletion in spawn_blocking (trash crate and std::fs are sync).
    let path_owned = args.path.clone();
    let permanent = args.permanent;

    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let p = std::path::Path::new(&path_owned);
        if permanent {
            // Permanent deletion: use std::fs directly.
            if p.is_dir() {
                std::fs::remove_dir_all(p)
                    .map_err(|e| format!("failed to delete directory: {}", e))
            } else {
                std::fs::remove_file(p)
                    .map_err(|e| format!("failed to delete file: {}", e))
            }
        } else {
            // Trash deletion: use the trash crate (handles files and dirs).
            trash::delete(p).map_err(|e| format!("failed to move to trash: {}", e))
        }
    })
    .await;

    // Step 3: Handle the spawn_blocking result.
    match result {
        Err(_panic) => {
            // The blocking task panicked — internal error.
            ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text(
                    "internal error: delete task panicked".to_string(),
                ),
                is_error: false,
                read_paths: None,
            }
        }
        Ok(Err(e)) => {
            // The deletion operation failed (trash or remove failed).
            ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text(e),
                is_error: false,
                read_paths: None,
            }
        }
        Ok(Ok(())) => {
            // Deletion succeeded — emit a plain-text success message.
            let verb = if permanent {
                "permanently deleted"
            } else {
                "moved to trash"
            };

            ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text(format!(
                    "{} {} ({})",
                    args.path, verb, kind_str
                )),
                is_error: false,
                read_paths: None,
            }
        }
    }
}
