//! Executor for the delete tool — handles all file I/O and deletion operations.
//!
//! This module contains the core logic for validating paths, checking file/directory existence,
//! and performing either trash or permanent deletion. All file I/O is async via tokio::fs,
//! and blocking operations (trash, remove_dir_all) are executed in spawn_blocking to avoid
//! blocking the async runtime.

use crate::args::DeleteArgs;
use crate::output::{DeleteOutput, DeletedKind};
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};

/// Executes the delete tool with the given arguments.
///
/// Validates that the path exists, determines whether it's a file or directory,
/// and then either moves it to trash (default, recoverable) or permanently deletes it
/// (if permanent: true). Blocking operations are executed in spawn_blocking to avoid
/// blocking the async runtime.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized delete arguments containing the path and permanent flag.
///
/// # Returns
/// A `ToolResult` with either success (JSON DeleteOutput) or failure (Text error message).
pub async fn execute(call_id: ToolCallId, args: DeleteArgs) -> ToolResult {
    let path = std::path::Path::new(&args.path);

    // Hey friend! Operon requires all filesystem tools to receive absolute paths.
    // This keeps the tool layer purely stateless and deterministic without relying
    // on process-wide current working directory (CWD) state.
    if !path.is_absolute() {
        return ToolResult {
            call_id,
            name: "delete".to_string(),
            content: ToolContent::Text(
                "Path must be an absolute path. Relative paths are not supported.".to_string(),
            ),
            is_error: true,
        };
    }

    // Step 1: Resolve path and determine kind (file or dir).
    // We need to verify the path exists and determine whether it's a file or directory
    // before attempting deletion.
    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text(format!("path does not exist: {}", args.path)),
                is_error: true,
            };
        }
        Err(e) => {
            return ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text(format!("failed to access path: {}: {}", args.path, e)),
                is_error: true,
            };
        }
    };

    // Determine whether the target is a file or directory.
    // is_dir() returns true for directories, false for files and symlinks.
    let kind = if meta.is_dir() {
        DeletedKind::Dir
    } else {
        DeletedKind::File // covers regular files and symlinks
    };

    // Step 2: Execute deletion inside spawn_blocking.
    // The trash crate is synchronous and may block. We must call it inside
    // spawn_blocking to avoid blocking the async runtime. Similarly, std::fs::remove_file
    // and std::fs::remove_dir_all are synchronous and should be in spawn_blocking.
    let path_owned = args.path.clone();
    let permanent = args.permanent;

    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let p = std::path::Path::new(&path_owned);
        if permanent {
            // Permanent deletion: use std::fs directly.
            if p.is_dir() {
                // For directories, recursively remove all contents.
                std::fs::remove_dir_all(p).map_err(|e| format!("failed to delete directory: {}", e))
            } else {
                // For files and symlinks, remove the file.
                std::fs::remove_file(p).map_err(|e| format!("failed to delete file: {}", e))
            }
        } else {
            // Trash deletion: use the trash crate.
            // The trash crate handles both files and directories automatically.
            trash::delete(p).map_err(|e| format!("failed to move to trash: {}", e))
        }
    })
    .await;

    // Step 3: Handle spawn_blocking result.
    // spawn_blocking returns Result<T, JoinError>. If the task panicked, we get Err.
    // If the task completed, we get Ok(result_from_closure).
    match result {
        Err(_panic) => {
            // The spawn_blocking task panicked — this is an internal error.
            return ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text("internal error: delete task panicked".to_string()),
                is_error: true,
            };
        }
        Ok(Err(e)) => {
            // The deletion operation failed (trash or remove failed).
            return ToolResult {
                call_id,
                name: "delete".to_string(),
                content: ToolContent::Text(e),
                is_error: true,
            };
        }
        Ok(Ok(())) => {
            // Deletion succeeded — fall through to success case.
        }
    }

    // Step 4: Return success.
    // Construct the output with the path, kind, permanent flag, and a human-readable message.
    let message = if permanent {
        format!(
            "Permanently deleted {} ({})",
            args.path,
            if kind == DeletedKind::Dir {
                "dir"
            } else {
                "file"
            }
        )
    } else {
        format!(
            "Moved {} to trash ({})",
            args.path,
            if kind == DeletedKind::Dir {
                "dir"
            } else {
                "file"
            }
        )
    };

    let output = DeleteOutput {
        path: args.path.clone(),
        kind,
        permanent,
        message,
    };

    ToolResult {
        call_id,
        name: "delete".to_string(),
        content: ToolContent::Json(serde_json::to_value(&output).unwrap_or_else(
            |e| serde_json::json!({ "error": format!("serialization bug: {}", e) }),
        )),
        is_error: false,
    }
}
