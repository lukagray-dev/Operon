//! Executor for the write tool — handles all file I/O and atomic writes.
//!
//! This module contains the core logic for validating paths, checking parent
//! directory existence, and atomically writing file content to disk using a
//! temp file + rename pattern. All file I/O is async via tokio::fs.

use crate::args::WriteArgs;
use crate::output::WriteOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};

/// Executes the write tool with the given arguments.
///
/// Validates the parent directory exists, determines if this is a create or overwrite,
/// and atomically writes the file content to disk using a temp file + rename pattern.
/// If the write fails at any point, the original file (if it existed) is NOT modified.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized write arguments containing the path and content.
///
/// # Returns
/// A `ToolResult` with either success (JSON WriteOutput) or failure (Text error message).
pub async fn execute(call_id: ToolCallId, args: WriteArgs) -> ToolResult {
    // Step 1: Check that the parent directory exists.
    // If it doesn't, return an error immediately without attempting to write.
    let path = std::path::Path::new(&args.path);

    let parent = path.parent().unwrap_or(std::path::Path::new("."));

    if !parent.exists() {
        return ToolResult {
            call_id,
            name: "write".to_string(),
            content: ToolContent::Text(format!(
                "parent directory does not exist: {}",
                parent.display()
            )),
            is_error: true,
        };
    }

    // Step 2: Determine if this is a create or overwrite by checking if the file exists.
    // This check happens before any write attempt, so we know the original state.
    let file_existed = path.exists();

    // Step 3: Atomic write using temp file + rename pattern.
    // Create a temp file in the same directory as the target to ensure same filesystem.
    // This guarantees that the rename operation is atomic on most filesystems.
    let tmp_path = parent.join(format!(
        ".operon_write_tmp_{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));

    // Write content to the temp file. If this fails, clean up the temp file and return error.
    if let Err(e) = tokio::fs::write(&tmp_path, args.content.as_bytes()).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return ToolResult {
            call_id,
            name: "write".to_string(),
            content: ToolContent::Text(format!(
                "failed to write file: {}. File was not modified.",
                e
            )),
            is_error: true,
        };
    }

    // Atomically rename the temp file to the target path.
    // If this fails, clean up the temp file and return error.
    // The original file (if it existed) remains untouched.
    if let Err(e) = tokio::fs::rename(&tmp_path, path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return ToolResult {
            call_id,
            name: "write".to_string(),
            content: ToolContent::Text(format!(
                "failed to finalize write: {}. File was not modified.",
                e
            )),
            is_error: true,
        };
    }

    // Step 4: Return success with metadata about the write operation.
    let bytes_written = args.content.len();
    let created = !file_existed;
    let message = if created {
        format!("Created {} ({} bytes)", args.path, bytes_written)
    } else {
        format!("Overwrote {} ({} bytes)", args.path, bytes_written)
    };

    let output = WriteOutput {
        path: args.path.clone(),
        created,
        bytes_written,
        message,
    };

    ToolResult {
        call_id,
        name: "write".to_string(),
        content: ToolContent::Text(output.to_plain_text()),
        is_error: false,
    }
}
