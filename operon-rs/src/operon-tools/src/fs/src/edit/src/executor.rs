//! Executor for the edit tool — handles all patch parsing, hunk matching, and atomic writes.
//!
//! Hey friend! This module contains the core execution engine for the edit tool.
//! It parses unified-diff patch strings into chunks, uses fuzzy sequence seeking
//! to locate matching context/lines within the file, applies replacements in-memory,
//! and atomically writes the modified content to disk using a temp file + rename.

use crate::args::EditArgs;
use crate::chunk_parser::{parse_patch_chunks, UpdateFileChunk};
use crate::output::EditOutput;
use crate::seek_sequence::seek_sequence;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};

/// Executes the edit tool with the given arguments.
///
/// Parses `args.patch` into hunks, reads `args.path`, matches and computes line
/// replacements in-memory, and atomically writes the result to disk.
/// If any hunk or parse step fails, the file is NOT modified.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args`: The deserialized edit arguments containing path and patch string.
///
/// # Returns
/// A `ToolResult` with either success (JSON EditOutput) or failure (Text error message).
pub async fn execute(call_id: ToolCallId, args: EditArgs) -> ToolResult {
    // Step 1: Parse the patch string into UpdateFileChunks.
    let chunks = match parse_patch_chunks(&args.patch) {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "edit".to_string(),
                content: ToolContent::Text(format!("failed to parse patch: {e}")),
                is_error: true,
            };
        }
    };

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

    // Step 3: Split content into line vector, stripping trailing carriage returns (\r).
    let is_crlf = content.contains("\r\n");
    let mut original_lines: Vec<String> = content.split('\n').map(String::from).collect();
    for line in &mut original_lines {
        if line.ends_with('\r') {
            line.pop();
        }
    }
    if original_lines.last().is_some_and(String::is_empty) {
        original_lines.pop();
    }

    // Step 4: Compute line replacements using seek_sequence matcher.
    let replacements = match compute_replacements(&original_lines, &args.path, &chunks) {
        Ok(r) => r,
        Err(err_msg) => {
            return ToolResult {
                call_id,
                name: "edit".to_string(),
                content: ToolContent::Text(err_msg),
                is_error: true,
            };
        }
    };

    // Step 5: Apply replacements to line vector.
    let mut new_lines = apply_replacements(original_lines, &replacements);
    if !new_lines.last().is_some_and(String::is_empty) {
        new_lines.push(String::new());
    }

    let line_separator = if is_crlf { "\r\n" } else { "\n" };
    let new_contents = new_lines.join(line_separator);

    // Step 6: Atomic write — only reached if ALL hunks matched and applied cleanly.
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

    if let Err(e) = tokio::fs::write(&tmp_path, new_contents.as_bytes()).await {
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

    // Step 7: Return success.
    let hunks_applied = chunks.len();
    let output = EditOutput {
        path: args.path.clone(),
        hunks_applied,
        message: format!("Applied {} edit(s) to {}", hunks_applied, args.path),
    };

    ToolResult {
        call_id,
        name: "edit".to_string(),
        content: ToolContent::Json(serde_json::to_value(&output).unwrap_or_else(
            |e| serde_json::json!({ "error": format!("serialization bug: {e}") }),
        )),
        is_error: false,
    }
}

/// Compute a list of line replacements needed to transform `original_lines` into new lines.
///
/// Ported from Codex's `lib.rs::compute_replacements` with enhanced context seeking.
/// Returns `(start_index, old_len, new_lines)` tuples for each chunk.
pub fn compute_replacements(
    original_lines: &[String],
    path: &str,
    chunks: &[UpdateFileChunk],
) -> Result<Vec<(usize, usize, Vec<String>)>, String> {
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut line_index: usize = 0;

    for (hunk_idx, chunk) in chunks.iter().enumerate() {
        // If a chunk has a `change_context`, locate it using seek_sequence or substring matching.
        if let Some(ctx_line) = &chunk.change_context {
            let found_ctx = seek_sequence(
                original_lines,
                std::slice::from_ref(ctx_line),
                line_index,
                /*eof*/ false,
            )
            .or_else(|| {
                // Substring fallback for partial context headers (e.g. `@@ fn old_name()` matching `fn old_name() {`)
                let trimmed_ctx = ctx_line.trim();
                if trimmed_ctx.is_empty() {
                    return None;
                }
                (line_index..original_lines.len())
                    .find(|&i| original_lines[i].trim().contains(trimmed_ctx))
            });

            if let Some(idx) = found_ctx {
                line_index = idx;
            }
            // If ctx_line is an informal comment/header not in the source file,
            // we proceed with line_index as-is and rely on old_lines matching.
        }

        if chunk.old_lines.is_empty() {
            // Pure addition (no old lines). Insert at end or before final trailing empty line.
            let insertion_idx = if original_lines.last().is_some_and(String::is_empty) {
                original_lines.len() - 1
            } else {
                original_lines.len()
            };
            replacements.push((insertion_idx, 0, chunk.new_lines.clone()));
            continue;
        }

        // Attempt to locate old_lines sequence starting from line_index.
        let mut pattern: &[String] = &chunk.old_lines;
        let mut found = seek_sequence(
            original_lines,
            pattern,
            line_index,
            chunk.is_end_of_file,
        );

        let mut new_slice: &[String] = &chunk.new_lines;

        // Sentinel retry: if pattern ends with an empty line (representing EOF newline), retry without it.
        if found.is_none() && pattern.last().is_some_and(String::is_empty) {
            pattern = &pattern[..pattern.len() - 1];
            if new_slice.last().is_some_and(String::is_empty) {
                new_slice = &new_slice[..new_slice.len() - 1];
            }

            found = seek_sequence(
                original_lines,
                pattern,
                line_index,
                chunk.is_end_of_file,
            );
        }

        if let Some(start_idx) = found {
            replacements.push((start_idx, pattern.len(), new_slice.to_vec()));
            line_index = start_idx + pattern.len();
        } else {
            return Err(format!(
                "hunk {hunk_idx}: old_string not found in file: {path}.\n\
                 The file may have changed since it was last read. Re-read the file and retry.\n\
                 Expected lines were:\n{}",
                chunk.old_lines.join("\n")
            ));
        }
    }

    replacements.sort_by_key(|(index, _, _)| *index);
    Ok(replacements)
}

/// Apply `(start_index, old_len, new_lines)` replacements to `original_lines`.
///
/// Ported from Codex's `lib.rs::apply_replacements`.
/// Applies replacements in descending index order to avoid line index shift.
pub fn apply_replacements(
    mut lines: Vec<String>,
    replacements: &[(usize, usize, Vec<String>)],
) -> Vec<String> {
    for (start_idx, old_len, new_segment) in replacements.iter().rev() {
        let start_idx = *start_idx;
        let old_len = *old_len;

        // Remove old lines.
        for _ in 0..old_len {
            if start_idx < lines.len() {
                lines.remove(start_idx);
            }
        }

        // Insert replacement lines.
        for (offset, new_line) in new_segment.iter().enumerate() {
            lines.insert(start_idx + offset, new_line.clone());
        }
    }

    lines
}
