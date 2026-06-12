//! Executor for the write tool — handles all file I/O and atomic writes.
//!
//! This module contains the core logic for optionally creating parent directories,
//! checking whether the file already existed, and atomically writing file content
//! to disk using a temp-file + rename pattern. All file I/O is async via tokio::fs.
//!
//! ## Output format (plain text, ToolContent::Text)
//!
//! Success (new file):   "{path} created"
//! Success (overwrite):  "{path} overwritten"
//! Any error:            "{path}\nERROR: {reason}"
//!
//! is_error is always false — the model reads the inline ERROR text.

use crate::args::WriteArgs;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Builds a plain-text ToolResult that signals an error to the model.
///
/// The format is "{path}\nERROR: {reason}" so the model can immediately identify
/// which file failed and why. is_error is intentionally false — the model reads
/// the inline ERROR text rather than relying on the is_error flag.
fn error_result(call_id: ToolCallId, path: &str, reason: &str) -> ToolResult {
    ToolResult {
        call_id,
        name: "write".to_string(),
        content: ToolContent::Text(format!("{}\nERROR: {}", path, reason)),
        is_error: false,
        read_paths: None,
    }
}

// ── Executor ───────────────────────────────────────────────────────────────────

/// Executes the write tool with the given arguments.
///
/// 1. Auto-creates parent directories if they don't exist.
/// 2. Records whether the file already existed (create vs overwrite).
/// 3. Atomically writes content via temp file + rename.
/// 4. Returns a plain-text ToolResult (success or error).
///
/// If the write fails at any point, the original file (if it existed) is NOT
/// modified because the temp file is cleaned up before returning the error.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`:    The parsed write arguments containing the path and content.
///
/// # Returns
/// A `ToolResult` with ToolContent::Text — never Json. is_error is always false.
pub async fn execute(call_id: ToolCallId, args: WriteArgs) -> ToolResult {
    let path = std::path::Path::new(&args.path);

    // Step 1: Auto-create parent directories.
    // Previously this returned an error if the parent didn't exist; now we
    // create it automatically so the model doesn't need a separate mkdir step.
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return error_result(
                    call_id,
                    &args.path,
                    &format!("failed to create parent directory: {}", e),
                );
            }
        }
    }

    // Step 2: Record whether the file already exists before any write attempt.
    // This determines whether the success message says "created" or "overwritten".
    let file_existed = path.exists();

    // Determine the parent for temp file placement. Fall back to "." if path
    // has no parent component (e.g., a bare filename — shouldn't happen in
    // practice since paths are required to be absolute).
    let parent = path.parent().unwrap_or(std::path::Path::new("."));

    // Step 3: Atomic write using temp file + rename.
    // Create the temp file in the SAME directory as the target so that the
    // rename is guaranteed to be on the same filesystem — making it atomic.
    let tmp_path = parent.join(format!(
        ".operon_write_tmp_{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));

    // Write content to the temp file. Clean up on failure so no stray temp
    // files are left behind.
    if let Err(e) = tokio::fs::write(&tmp_path, args.content.as_bytes()).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return error_result(
            call_id,
            &args.path,
            &format!("failed to write: {}", e),
        );
    }

    // Atomically rename the temp file to the final target path.
    // If rename fails, clean up the temp file. The original file is untouched.
    if let Err(e) = tokio::fs::rename(&tmp_path, path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return error_result(
            call_id,
            &args.path,
            &format!("failed to finalize write: {}", e),
        );
    }

    // Step 4: Return plain-text success.
    // "created" for new files, "overwritten" for existing files.
    let verb = if file_existed { "overwritten" } else { "created" };
    ToolResult {
        call_id,
        name: "write".to_string(),
        content: ToolContent::Text(format!("{} {}", args.path, verb)),
        is_error: false,
        read_paths: None,
    }
}
