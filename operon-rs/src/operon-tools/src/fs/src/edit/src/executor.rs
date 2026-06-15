//! Executor for the edit tool — file I/O, hunk application, and atomic writes.
//!
//! This module contains the core logic for:
//!   1. Reading the target file and normalising line endings.
//!   2. Applying each hunk using `seek_sequence` to locate the region.
//!   3. Collecting and validating replacements (overlap check).
//!   4. Applying replacements in reverse-index order (avoids index shift).
//!   5. Atomically writing the result to disk (temp file + rename).
//!
//! All file I/O is async via tokio::fs.
//! All error conditions are returned as Ok(ToolResult) with is_error=false —
//! the inline text carries the error message for the model to read.

use crate::args::EditArgs;
use crate::seek_sequence::seek_sequence;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};

// ── Helper ─────────────────────────────────────────────────────────────────────

/// Build a plain-text ToolResult that signals an inline error to the model.
///
/// is_error is intentionally false — the model reads the inline text rather
/// than relying on the is_error flag. This mirrors the write/append pattern.
fn error_result(call_id: ToolCallId, msg: &str) -> ToolResult {
    ToolResult {
        call_id,
        name: "edit".to_string(),
        content: ToolContent::Text(msg.to_string()),
        is_error: false,
        read_paths: None,
    }
}

// ── Pending replacement record ─────────────────────────────────────────────────

/// A replacement to apply to the in-memory line buffer.
///
/// Collected during the hunk-application phase, then sorted and validated
/// before being applied in reverse order to avoid index shifting.
struct Replacement {
    /// 0-based index of the first line to replace in the original line buffer.
    start_idx: usize,

    /// Number of lines in the original buffer to remove (old_lines.len()).
    old_len: usize,

    /// New lines to insert at `start_idx` (may be empty for pure deletion).
    new_lines: Vec<String>,

    /// 1-based hunk index (for error messages).
    hunk_number: usize,
}

// ── Executor ───────────────────────────────────────────────────────────────────

/// Execute the edit tool: parse the file, apply all hunks, write atomically.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args`:    The parsed EditArgs containing the path and hunks.
///
/// # Returns
/// A `ToolResult` with ToolContent::Text for both success and error.
/// is_error is always false — the model reads inline text.
pub async fn execute(call_id: ToolCallId, args: EditArgs) -> ToolResult {
    let path = &args.path;

    // ── Step 1: Read the file ──────────────────────────────────────────────
    let raw_content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) => {
            return error_result(
                call_id,
                &format!("{}\nfailed to read file: {}", path, e),
            );
        }
    };

    // ── Step 2: Normalise CRLF → LF ───────────────────────────────────────
    // Work on a unified LF-only representation so the seek and replacement
    // logic is not confused by mixed line endings (common on Windows).
    let had_trailing_newline = raw_content.ends_with('\n');
    let content = raw_content.replace("\r\n", "\n").replace('\r', "\n");

    // ── Step 3: Split into lines ───────────────────────────────────────────
    // Split on '\n'. After normalisation, `content.split('\n')` will produce
    // one trailing empty string if the file ended with a newline — drop it
    // so line indices match visual line numbers.
    let mut lines: Vec<String> = content.split('\n').map(String::from).collect();

    // Drop the trailing empty element produced by a file-ending newline.
    if had_trailing_newline {
        if let Some(last) = lines.last() {
            if last.is_empty() {
                lines.pop();
            }
        }
    }

    // ── Step 4: Process hunks and collect replacements ─────────────────────
    // We scan through the hunks in order, advancing `line_index` as each hunk
    // is located. The replacements are NOT applied yet — we collect them all
    // first so we can check for overlaps before touching the buffer.

    let mut line_index: usize = 0; // cursor: next search starts from here
    let mut replacements: Vec<Replacement> = Vec::with_capacity(args.hunks.len());

    for (hunk_idx, hunk) in args.hunks.iter().enumerate() {
        // 1-based for human-readable error messages.
        let hunk_number = hunk_idx + 1;



        // ── 4a: Seek anchor ───────────────────────────────────────────────
        // Hey friend! A seek context is a hint. We relax this search to perform a
        // case-insensitive substring search (i.e. if the file line contains the anchor text).
        // First we scan forward from `line_index`, and if not found, we fall back to scanning
        // from the beginning of the file.
        let mut seek_context_matched = false;
        if let Some(ref ctx) = hunk.seek_context {
            let normalized_ctx = ctx.trim().to_lowercase();
            if !normalized_ctx.is_empty() {
                // First pass: scan forward from cursor
                for i in line_index..lines.len() {
                    if lines[i].to_lowercase().contains(&normalized_ctx) {
                        line_index = i + 1;
                        seek_context_matched = true;
                        break;
                    }
                }
                // Fallback pass: scan the whole file from the beginning
                if !seek_context_matched {
                    for i in 0..line_index {
                        if lines[i].to_lowercase().contains(&normalized_ctx) {
                            line_index = i + 1;
                            seek_context_matched = true;
                            break;
                        }
                    }
                }
            }
        }

        // ── 4b: Pure insertion (empty old_lines) ──────────────────────────
        // When old_lines is empty the hunk is a pure insertion — it adds new
        // lines without removing anything. This is equivalent to inserting at
        // the current line_index position.
        if hunk.old_lines.is_empty() {
            if hunk.seek_context.is_some() && !seek_context_matched {
                let ctx = hunk.seek_context.as_ref().unwrap();
                return error_result(
                    call_id,
                    &format!(
                        "{}\nhunk {}: seek context not found: {}",
                        path, hunk_number, ctx
                    ),
                );
            }

            // For EOF-anchored insertions, place after the last real content
            // (before the implicit trailing newline position).
            let insert_at = if hunk.is_end_of_file {
                lines.len()
            } else {
                line_index
            };
            replacements.push(Replacement {
                start_idx: insert_at,
                old_len: 0,
                new_lines: hunk.new_lines.clone(),
                hunk_number,
            });
            continue;
        }

        // ── 4c: Seek the old_lines region ─────────────────────────────────
        let start_idx = if hunk.seek_context.is_some() && !seek_context_matched {
            // Seek context was provided but not found (likely a comment).
            // Search the entire file starting from index 0. If it matches exactly once,
            // we proceed using that index.
            let mut matches = Vec::new();
            let mut scan_idx = 0;
            while scan_idx <= lines.len().saturating_sub(hunk.old_lines.len()) {
                if let Some(m_idx) = seek_sequence(&lines, &hunk.old_lines, scan_idx, false) {
                    matches.push(m_idx);
                    scan_idx = m_idx + 1;
                } else {
                    break;
                }
            }

            if matches.len() == 1 {
                matches[0]
            } else {
                let ctx = hunk.seek_context.as_ref().unwrap();
                return error_result(
                    call_id,
                    &format!(
                        "{}\nhunk {}: seek context not found: {}",
                        path, hunk_number, ctx
                    ),
                );
            }
        } else {
            // Seek context either not provided, or matched successfully.
            // Search starting from the current line_index.
            let start_idx = match seek_sequence(&lines, &hunk.old_lines, line_index, hunk.is_end_of_file) {
                Some(idx) => idx,
                None => {
                    return error_result(
                        call_id,
                        &format!(
                            "{}\nhunk {}: match not found\nexpected:\n{}",
                            path,
                            hunk_number,
                            hunk.old_lines.join("\n")
                        ),
                    );
                }
            };

            // Check for multiple matches in the searchable range starting from line_index
            // to detect ambiguity.
            let mut matches = Vec::new();
            let mut scan_idx = line_index;
            while scan_idx <= lines.len().saturating_sub(hunk.old_lines.len()) {
                if let Some(m_idx) = seek_sequence(&lines, &hunk.old_lines, scan_idx, false) {
                    matches.push(m_idx);
                    scan_idx = m_idx + 1;
                } else {
                    break;
                }
            }

            if matches.len() > 1 {
                return error_result(
                    call_id,
                    &format!(
                        "{}\nhunk {}: matched {} times",
                        path, hunk_number, matches.len()
                    ),
                );
            }

            start_idx
        };

        // Found the region and verified it is unambiguous — record the replacement.
        replacements.push(Replacement {
            start_idx,
            old_len: hunk.old_lines.len(),
            new_lines: hunk.new_lines.clone(),
            hunk_number,
        });

        // Advance the cursor past the matched region so the next hunk
        // searches forward from here.
        line_index = start_idx + hunk.old_lines.len();
    }

    // ── Step 5: Sort and overlap check ────────────────────────────────────
    // Sort by start_idx ascending so we can detect overlaps with a single
    // linear pass. Overlapping hunks would produce undefined output — reject
    // them explicitly rather than silently producing corrupt output.
    replacements.sort_by_key(|r| r.start_idx);

    for window in replacements.windows(2) {
        let a = &window[0];
        let b = &window[1];
        // Overlap: b starts before a's region ends.
        if b.start_idx < a.start_idx + a.old_len {
            return error_result(
                call_id,
                &format!(
                    "{}\nhunk {} and hunk {}: overlapping matches",
                    path, a.hunk_number, b.hunk_number
                ),
            );
        }
    }

    // ── Step 6: Apply replacements in reverse order ────────────────────────
    // Processing from the end backwards means each splice does not shift the
    // indices of earlier replacements, so all start_idx values remain valid.
    let hunk_count = replacements.len();
    for rep in replacements.into_iter().rev() {
        // Remove `old_len` lines starting at `start_idx` then insert new ones.
        // `Vec::splice` handles both the removal and the insertion atomically
        // in the in-memory buffer.
        lines.splice(
            rep.start_idx..rep.start_idx + rep.old_len,
            rep.new_lines,
        );
    }

    // ── Step 7: Reassemble the file content ───────────────────────────────
    // Join all lines back together. Restore the trailing newline if the
    // original file had one.
    let mut new_content = lines.join("\n");
    if had_trailing_newline {
        new_content.push('\n');
    }

    // ── Step 8: Atomic write (temp file + rename) ──────────────────────────
    // Write to a temp file in the SAME directory as the target so the rename
    // is guaranteed to be on the same filesystem — making it atomic.
    let parent = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("."));

    let tmp_path = parent.join(format!(
        ".operon_edit_tmp_{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));

    // Write content to the temp file; clean up on failure.
    if let Err(e) = tokio::fs::write(&tmp_path, new_content.as_bytes()).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return error_result(
            call_id,
            &format!(
                "{}\nfailed to write temp file: {}. File was not modified.",
                path, e
            ),
        );
    }

    // Atomically rename temp file to the final target path.
    if let Err(e) = tokio::fs::rename(&tmp_path, path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return error_result(
            call_id,
            &format!(
                "{}\nfailed to rename temp file to target: {}. File was not modified.",
                path, e
            ),
        );
    }

    // ── Step 9: Return success ─────────────────────────────────────────────
    ToolResult {
        call_id,
        name: "edit".to_string(),
        content: ToolContent::Text(format!(
            "{} ({} hunk(s) applied)",
            path, hunk_count
        )),
        is_error: false,
        read_paths: None,
    }
}
