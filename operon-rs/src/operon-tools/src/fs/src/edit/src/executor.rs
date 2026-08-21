//! Executor for the edit tool — handles hunk matching, sequential in-memory edits, and atomic writes.
//!
//! Hey friend! This module contains the core execution engine for the edit tool.
//! It processes an array of `old_string` -> `new_string` hunks sequentially against an in-memory
//! working buffer.
//!
//! Key capabilities:
//! 1. Exact-substring matching (fast, byte-precise).
//! 2. 6-pass fuzzy line matching via `seek_sequence` when exact matching fails (tolerates
//!    indentation changes, trailing whitespace, Unicode quotes/dashes, and casing differences).
//! 3. Ambiguity detection: zero matches = not found; multiple matches = ambiguous error.
//! 4. Partial-success execution: valid matching hunks are applied and committed to disk,
//!    while failed hunks are skipped and reported in structured diagnostics.
//! 5. Atomic disk writes using a temporary file in the target directory followed by an atomic rename.

use crate::args::EditArgs;
use crate::output::{EditOutput, HunkFailure};
use crate::seek_sequence::{find_sequence_match, SequenceMatch};
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};

/// Executes the edit tool with the provided arguments.
///
/// Pre-validates the edits array, reads the file, processes each hunk sequentially
/// against in-memory working content, and atomically writes the modified content
/// to disk if at least one hunk succeeded.
///
/// # Arguments
/// - `call_id`: Unique identifier for this tool call.
/// - `args`: Deserialized edit arguments containing `path` and `edits`.
///
/// # Returns
/// A `ToolResult` containing structured `EditOutput` JSON.
pub async fn execute(call_id: ToolCallId, args: EditArgs) -> ToolResult {
    let path = std::path::Path::new(&args.path);

    // Hey friend! Operon requires all filesystem tools to receive absolute paths.
    // This keeps the tool layer purely stateless and deterministic without relying
    // on process-wide current working directory (CWD) state.
    if !path.is_absolute() {
        return ToolResult {
            call_id,
            name: "edit".to_string(),
            content: ToolContent::Text(
                "Path must be an absolute path. Relative paths are not supported.".to_string(),
            ),
            is_error: true,
        };
    }

    // Step 1: Pre-validate input structure (fast-fail before touching disk).
    // An empty edits array is an invalid tool call.
    if args.edits.is_empty() {
        return ToolResult {
            call_id,
            name: "edit".to_string(),
            content: ToolContent::Text("edits array must contain at least one hunk".to_string()),
            is_error: true,
        };
    }

    // Pre-validate that no hunk has old_string identical to new_string.
    for (i, hunk) in args.edits.iter().enumerate() {
        if hunk.old_string == hunk.new_string {
            return ToolResult {
                call_id,
                name: "edit".to_string(),
                content: ToolContent::Text(format!(
                    "hunk {i}: old_string and new_string are identical — no change would be made"
                )),
                is_error: true,
            };
        }
    }

    // Step 2: Read the target file.
    let content = match tokio::fs::read_to_string(&args.path).await {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "edit".to_string(),
                content: ToolContent::Text(format!("failed to read file: {}: {}", args.path, e)),
                is_error: true,
            };
        }
    };

    // Step 3: Sequentially apply hunks to an in-memory working buffer.
    let total_hunks = args.edits.len();
    let mut working = content;
    let mut hunks_applied = 0;
    let mut hunks_failed = 0;
    let mut failures = Vec::new();

    for (i, hunk) in args.edits.into_iter().enumerate() {
        // Match Strategy A: Exact substring check (byte-precise, handles partial-line edits).
        let exact_count = count_occurrences(&working, &hunk.old_string);

        if exact_count == 1 {
            // Unique exact match found: apply replacement directly.
            working = working.replacen(&hunk.old_string, &hunk.new_string, 1);
            hunks_applied += 1;
            continue;
        } else if exact_count > 1 {
            // Ambiguous exact matches: record error and skip this hunk.
            hunks_failed += 1;
            failures.push(HunkFailure {
                hunk_index: i,
                old_string: hunk.old_string,
                reason: format!(
                    "old_string matched {exact_count} times — ambiguous. \
                     Include more surrounding context lines in old_string to make it unique."
                ),
            });
            continue;
        }

        // Match Strategy B: 6-pass fuzzy sequence matching via seek_sequence.
        // Used when exact substring match count is 0 (handles indentation, whitespace, Unicode, casing drift).
        let is_crlf = working.contains("\r\n");
        let line_separator = if is_crlf { "\r\n" } else { "\n" };

        let mut working_lines: Vec<String> = working
            .split('\n')
            .map(|s| s.trim_end_matches('\r').to_string())
            .collect();
        let has_trailing_empty = working_lines.last().is_some_and(String::is_empty);
        if has_trailing_empty {
            working_lines.pop();
        }

        let mut pattern_lines: Vec<String> = hunk
            .old_string
            .split('\n')
            .map(|s| s.trim_end_matches('\r').to_string())
            .collect();
        if pattern_lines.last().is_some_and(String::is_empty) && pattern_lines.len() > 1 {
            pattern_lines.pop();
        }

        match find_sequence_match(&working_lines, &pattern_lines) {
            SequenceMatch::Unique(start_line) => {
                // Unique fuzzy match located: replace lines in working buffer.
                let mut new_lines: Vec<String> = hunk
                    .new_string
                    .split('\n')
                    .map(|s| s.trim_end_matches('\r').to_string())
                    .collect();
                if new_lines.last().is_some_and(String::is_empty) && new_lines.len() > 1 {
                    new_lines.pop();
                }

                // Remove the matched old lines.
                for _ in 0..pattern_lines.len() {
                    if start_line < working_lines.len() {
                        working_lines.remove(start_line);
                    }
                }

                // Insert the replacement lines.
                for (offset, new_line) in new_lines.into_iter().enumerate() {
                    working_lines.insert(start_line + offset, new_line);
                }

                if has_trailing_empty && !working_lines.last().is_some_and(String::is_empty) {
                    working_lines.push(String::new());
                }

                working = working_lines.join(line_separator);
                hunks_applied += 1;
            }
            SequenceMatch::Ambiguous(count) => {
                hunks_failed += 1;
                failures.push(HunkFailure {
                    hunk_index: i,
                    old_string: hunk.old_string,
                    reason: format!(
                        "old_string matched {count} times — ambiguous. \
                         Include more surrounding context lines in old_string to make it unique."
                    ),
                });
            }
            SequenceMatch::NotFound => {
                hunks_failed += 1;
                failures.push(HunkFailure {
                    hunk_index: i,
                    old_string: hunk.old_string,
                    reason: "old_string not found in file. \
                             The file may have changed since it was last read. \
                             Re-read the file and retry."
                        .to_string(),
                });
            }
        }
    }

    // Step 4: Atomic write to disk — only if at least one hunk succeeded.
    if hunks_applied > 0 {
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

        // Write modified content to temporary file.
        if let Err(e) = tokio::fs::write(&tmp_path, working.as_bytes()).await {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return ToolResult {
                call_id,
                name: "edit".to_string(),
                content: ToolContent::Text(format!(
                    "failed to write temp file: {e}. File was not modified."
                )),
                is_error: true,
            };
        }

        // Atomically rename temporary file to target path.
        if let Err(e) = tokio::fs::rename(&tmp_path, &args.path).await {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return ToolResult {
                call_id,
                name: "edit".to_string(),
                content: ToolContent::Text(format!(
                    "failed to rename temp file to target: {e}. File was not modified."
                )),
                is_error: true,
            };
        }
    }

    // Step 5: Construct structured outcome.
    let (is_error, message) = if hunks_failed == 0 {
        // Complete success: all hunks applied cleanly.
        (
            false,
            format!("Applied {} edit(s) to {}", hunks_applied, args.path),
        )
    } else if hunks_applied > 0 {
        // Partial success: some hunks succeeded and were written; some failed.
        (
            true,
            format!(
                "Partially applied: {} of {} edit(s) written to {}; {} edit(s) failed.",
                hunks_applied, total_hunks, args.path, hunks_failed
            ),
        )
    } else {
        // Complete failure: no hunks applied, disk untouched.
        (
            true,
            format!(
                "Failed to apply any edits to {}. File was not modified.",
                args.path
            ),
        )
    };

    let output = EditOutput {
        path: args.path,
        total_hunks,
        hunks_applied,
        hunks_failed,
        failures,
        message,
    };

    ToolResult {
        call_id,
        name: "edit".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(
                |e| serde_json::json!({ "error": format!("serialization bug: {e}") }),
            ),
        ),
        is_error,
    }
}

/// Counts non-overlapping occurrences of `needle` in `haystack`.
///
/// Returns 0 for an empty needle. Used for exact-string detection.
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
