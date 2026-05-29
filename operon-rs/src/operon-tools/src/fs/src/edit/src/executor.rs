//! Executor for the edit tool — handles all file I/O, hunk application, and atomic writes.
//!
//! This module contains the core logic for validating hunks, reading files, applying
//! exact-string replacements in order, and atomically writing the result to disk.
//! All file I/O is async via tokio::fs.

use crate::args::EditArgs;
use crate::output::EditOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};

/// Executes the edit tool with the given arguments.
///
/// Validates hunks, reads the file, applies all edits in order on the in-memory content,
/// and atomically writes the result to disk. If any hunk fails, the file is NOT modified.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized edit arguments containing the path and hunks.
///
/// # Returns
/// A `ToolResult` with either success (JSON EditOutput) or failure (Text error message).
pub async fn execute(call_id: ToolCallId, args: EditArgs) -> ToolResult {
    // Step 1: Validate that edits array is non-empty (fast fail, before reading file).
    if args.edits.is_empty() {
        return ToolResult {
            call_id,
            name: "edit".to_string(),
            content: ToolContent::Text(
                "edits array must contain at least one hunk".to_string(),
            ),
            is_error: true,
        };
    }

    // Step 2: Pre-validate that no hunk has old_string == new_string (fast fail).
    for (i, hunk) in args.edits.iter().enumerate() {
        if hunk.old_string == hunk.new_string {
            return ToolResult {
                call_id,
                name: "edit".to_string(),
                content: ToolContent::Text(format!(
                    "hunk {}: old_string and new_string are identical — no change would be made",
                    i
                )),
                is_error: true,
            };
        }
    }

    // Step 3: Read the file.
    let content = match tokio::fs::read_to_string(&args.path).await {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "edit".to_string(),
                content: ToolContent::Text(format!(
                    "failed to read file: {}: {}",
                    args.path, e
                )),
                is_error: true,
            };
        }
    };

    // Step 4: Apply hunks in order on a working string.
    let mut working = content;

    for (i, hunk) in args.edits.iter().enumerate() {
        let match_count = count_occurrences(&working, &hunk.old_string);
        match match_count {
            0 => {
                return ToolResult {
                    call_id,
                    name: "edit".to_string(),
                    content: ToolContent::Text(format!(
                        "hunk {}: old_string not found in file. \
                         The file may have changed since it was last read. \
                         Re-read the file and retry.\n\
                         old_string was:\n{}",
                        i, hunk.old_string
                    )),
                    is_error: true,
                };
            }
            1 => {
                // Exactly one match — apply the replacement.
                working = working.replacen(&hunk.old_string, &hunk.new_string, 1);
            }
            n => {
                return ToolResult {
                    call_id,
                    name: "edit".to_string(),
                    content: ToolContent::Text(format!(
                        "hunk {}: old_string matched {} times — ambiguous. \
                         Include more surrounding context lines in old_string \
                         to make it unique.\n\
                         old_string was:\n{}",
                        i, n, hunk.old_string
                    )),
                    is_error: true,
                };
            }
        }
    }

    // Step 5: Atomic write — only reached if ALL hunks applied successfully.
    // Create a temp file in the same directory as the target to ensure same filesystem.
    let parent = std::path::Path::new(&args.path)
        .parent()
        .unwrap_or(std::path::Path::new("."));

    let tmp_path = parent.join(format!(
        ".operon_edit_tmp_{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));

    // Write to temp file.
    if let Err(e) = tokio::fs::write(&tmp_path, working.as_bytes()).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return ToolResult {
            call_id,
            name: "edit".to_string(),
            content: ToolContent::Text(format!(
                "failed to write temp file: {}. File was not modified.",
                e
            )),
            is_error: true,
        };
    }

    // Atomically rename temp file to target.
    if let Err(e) = tokio::fs::rename(&tmp_path, &args.path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return ToolResult {
            call_id,
            name: "edit".to_string(),
            content: ToolContent::Text(format!(
                "failed to rename temp file to target: {}. File was not modified.",
                e
            )),
            is_error: true,
        };
    }

    // Step 6: Return success.
    let hunks_applied = args.edits.len();
    let output = EditOutput {
        path: args.path.clone(),
        hunks_applied,
        message: format!("Applied {} edit(s) to {}", hunks_applied, args.path),
    };

    ToolResult {
        call_id,
        name: "edit".to_string(),
        content: ToolContent::Json(serde_json::to_value(&output).unwrap_or_else(|e| {
            serde_json::json!({ "error": format!("serialization bug: {}", e) })
        })),
        is_error: false,
    }
}

/// Counts non-overlapping occurrences of `needle` in `haystack`.
///
/// Returns 0 for empty needle (treated as not found).
/// Used to validate that old_string matches exactly once before applying a replacement.
///
/// # Arguments
/// - `haystack`: The string to search in.
/// - `needle`: The string to search for.
///
/// # Returns
/// The number of non-overlapping occurrences of needle in haystack.
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        count += 1;
        start += pos + needle.len();
    }
    count
}
