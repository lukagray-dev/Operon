//! Executor for the append tool — handles all file I/O and atomic appends.
//!
//! This module contains the core logic for validating paths, checking file existence,
//! and atomically appending content to the end of a file using append mode.
//! All file I/O is async via tokio::fs.

use crate::args::AppendArgs;
use crate::output::AppendOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use tokio::io::AsyncWriteExt;

/// Executes the append tool with the given arguments.
///
/// Validates that the file exists and is not a directory, checks that content is
/// non-empty, and atomically appends the content to the end of the file using
/// append mode. The append operation positions the write cursor at EOF at the OS
/// level, ensuring atomicity without requiring a temp file or reading the existing
/// content.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized append arguments containing the path and content.
///
/// # Returns
/// A `ToolResult` with either success (JSON AppendOutput) or failure (Text error message).
pub async fn execute(call_id: ToolCallId, args: AppendArgs) -> ToolResult {
    // Step 1: Reject empty content (fast fail).
    // Appending empty content is a no-op and indicates a mistake by the model.
    if args.content.is_empty() {
        return ToolResult {
            call_id,
            name: "append".to_string(),
            content: ToolContent::Text("content is empty — nothing to append".to_string()),
            is_error: true,
        };
    }

    // Step 2: Check file exists and is not a directory.
    // We need to verify the file exists before attempting to append, and ensure
    // it's a regular file (or symlink), not a directory.
    let path = std::path::Path::new(&args.path);

    match tokio::fs::metadata(path).await {
        Ok(meta) => {
            // File exists. Check if it's a directory.
            if meta.is_dir() {
                return ToolResult {
                    call_id,
                    name: "append".to_string(),
                    content: ToolContent::Text(format!(
                        "path is a directory, not a file: {}",
                        args.path
                    )),
                    is_error: true,
                };
            }
            // is_file() or is_symlink() — proceed to append.
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File does not exist. Return error and suggest using write tool.
            return ToolResult {
                call_id,
                name: "append".to_string(),
                content: ToolContent::Text(format!(
                    "file does not exist: {}. Use the write tool to create new files.",
                    args.path
                )),
                is_error: true,
            };
        }
        Err(e) => {
            // Other metadata access error (permission denied, etc.).
            return ToolResult {
                call_id,
                name: "append".to_string(),
                content: ToolContent::Text(format!("failed to access file: {}: {}", args.path, e)),
                is_error: true,
            };
        }
    }

    // Step 3: Open with append mode and write.
    // Use tokio::fs::OpenOptions with .append(true). This positions the write
    // cursor at EOF at the OS level, atomically. No temp file needed — append
    // mode with O_APPEND is atomic per POSIX for writes under the pipe buffer
    // size, and safe for all practical agent use cases.
    let mut file = match tokio::fs::OpenOptions::new().append(true).open(path).await {
        Ok(f) => f,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "append".to_string(),
                content: ToolContent::Text(format!("failed to append to file: {}", e)),
                is_error: true,
            };
        }
    };

    // Write all bytes from the content string to the file.
    if let Err(e) = file.write_all(args.content.as_bytes()).await {
        return ToolResult {
            call_id,
            name: "append".to_string(),
            content: ToolContent::Text(format!("failed to append to file: {}", e)),
            is_error: true,
        };
    }

    // Flush to ensure all bytes are written before reading metadata.
    // This guarantees that the metadata read in the next step reflects the
    // appended content.
    if let Err(e) = file.flush().await {
        return ToolResult {
            call_id,
            name: "append".to_string(),
            content: ToolContent::Text(format!("failed to flush file after append: {}", e)),
            is_error: true,
        };
    }

    // Step 4: Read total file size (non-fatal if it fails).
    // After a successful append, we want to report the total file size.
    // If metadata fetch fails, we use 0 as a fallback — the append succeeded,
    // so we don't fail the entire operation just because we can't read metadata.
    let total_bytes = tokio::fs::metadata(path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    // Step 5: Return success.
    // Construct the output with the number of bytes appended and the total file size.
    let bytes_appended = args.content.len();

    let output = AppendOutput {
        path: args.path.clone(),
        bytes_appended,
        total_bytes,
        message: format!(
            "Appended {} bytes to {} (total: {} bytes)",
            bytes_appended, args.path, total_bytes
        ),
    };

    ToolResult {
        call_id,
        name: "append".to_string(),
        content: ToolContent::Text(output.to_plain_text()),
        is_error: false,
    }
}
