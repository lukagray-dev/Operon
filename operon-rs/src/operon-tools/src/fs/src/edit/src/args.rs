//! Argument types for the edit tool.
//!
//! This module defines the manual parsing logic for the edit tool's input.
//! Arguments arrive as a serde_json::Value where:
//!   - `args_json["path"]`     — the absolute file path from the `path` XML attr.
//!   - `args_json["__body__"]` — the raw diff body between `<<<<` and `>>>>`,
//!                               injected by the dispatcher.
//!
//! The body is a unified-diff-style hunk format:
//!   @@                   → hunk separator (no seek anchor)
//!   @@ some context text → hunk separator with seek anchor
//!   -line                → line to remove (present in old, absent in new)
//!   +line                → line to add (absent in old, present in new)
//!    line                → context line (space prefix; present in both old and new)
//!
//! No serde Deserialize is used — all parsing is done manually so we can
//! produce precise, actionable error messages for the model.

/// A single seek-and-replace hunk parsed from the edit body.
///
/// Each hunk describes a contiguous region of the file to locate and replace.
/// The `seek_sequence` algorithm locates `old_lines` in the file; the matched
/// region is then replaced with `new_lines`.
pub struct EditHunk {
    /// Optional single-line anchor for the seek_sequence search.
    ///
    /// Extracted from the text after "@@ " (trimmed). If the @@ separator
    /// has no trailing text, this is None and the search continues from the
    /// last match position.
    pub seek_context: Option<String>,

    /// Lines to find in the file (the '-' and ' ' prefixed lines, stripped).
    ///
    /// Context lines (' ' prefix) are included in both `old_lines` and
    /// `new_lines` so they participate in the seek and are preserved in output.
    pub old_lines: Vec<String>,

    /// Lines to write in place of the matched region (the '+' and ' ' lines).
    ///
    /// Context lines (' ' prefix) are included in both `old_lines` and
    /// `new_lines` so they are emitted as-is in the output.
    pub new_lines: Vec<String>,

    /// When true, `seek_sequence` is asked to anchor the match at the end of
    /// the file (eof=true). Set by a trailing "*** End of File" or "@@ EOF"
    /// marker on the last line of the hunk.
    pub is_end_of_file: bool,
}

/// Parsed arguments for the edit tool.
pub struct EditArgs {
    /// Absolute path to the file to edit (from the `path` XML attribute).
    pub path: String,

    /// One or more hunks to apply, in order, to the file.
    pub hunks: Vec<EditHunk>,
}

impl EditArgs {
    /// Parse EditArgs from the raw args_json Value injected by the dispatcher.
    ///
    /// Returns `Ok(EditArgs)` on success or `Err(String)` with a human-readable
    /// reason if required fields are missing, malformed, or the body produces
    /// no valid hunks.
    ///
    /// # Parsing rules
    /// - `path`:     required; must be a non-empty string.
    /// - `__body__`: required; must be a non-empty string containing at least one hunk.
    pub fn parse(args_json: &serde_json::Value) -> Result<EditArgs, String> {
        // ── Step 1: Extract the path attribute ────────────────────────────
        // The "path" attr is mandatory. Absence or a non-string value is a
        // hard error — we cannot proceed without knowing the target file.
        let path = args_json
            .get("path")
            .or_else(|| args_json.get("paths"))
            .ok_or_else(|| "missing or non-string attr: path".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'path' must be a string".to_string())?
            .trim()
            .to_string();

        if path.is_empty() {
            return Err("path is empty".to_string());
        }

        // ── Step 2: Extract the body ──────────────────────────────────────
        // The body is injected by the dispatcher under "__body__". Its absence
        // means the model sent a call without any diff content — also a hard
        // error, since there is nothing to apply.
        let body = args_json["__body__"]
            .as_str()
            .ok_or_else(|| "missing __body__: no diff content provided".to_string())?;

        // ── Step 3: Parse the body into hunks ─────────────────────────────
        let hunks = parse_hunks(body)?;

        Ok(EditArgs { path, hunks })
    }
}

/// Parse a diff body string into a list of `EditHunk` values.
///
/// The body is split on newlines and scanned line-by-line. A line beginning
/// with "@@" opens a new hunk. Lines beginning with '-', '+', or ' ' (space)
/// contribute to the current hunk's old_lines and/or new_lines. Empty lines
/// between hunks are silently skipped.
///
/// Returns an error if:
///  - A non-empty, non-@@ line appears that does not start with '-', '+', or ' '.
///  - No valid (non-empty) hunks are produced.
fn parse_hunks(body: &str) -> Result<Vec<EditHunk>, String> {
    // Each hunk accumulates its state here while we scan through the lines.
    // `current` is None until the first @@ line is encountered.
    let mut hunks: Vec<EditHunk> = Vec::new();

    // Working state for the hunk currently being built.
    let mut seek_context: Option<String> = None;
    let mut old_lines: Vec<String> = Vec::new();
    let mut new_lines: Vec<String> = Vec::new();
    let mut is_end_of_file = false;
    let mut in_hunk = false; // true once we have seen the first @@ line

    for raw_line in body.lines() {
        if raw_line.starts_with("@@") {
            // ── @@ separator: flush the current hunk, start a new one ──────
            if in_hunk {
                // Flush: only push if it has at least one real line.
                if !old_lines.is_empty() || !new_lines.is_empty() {
                    hunks.push(EditHunk {
                        seek_context,
                        old_lines,
                        new_lines,
                        is_end_of_file,
                    });
                }
                // Reset working state for the next hunk.
                old_lines = Vec::new();
                new_lines = Vec::new();
                is_end_of_file = false;
            }
            in_hunk = true;

            // Extract the optional seek-anchor text after "@@ ".
            // "@@ " followed by nothing → None; "@@ some text" → Some("some text").
            let anchor_text = raw_line["@@".len()..].trim();
            seek_context = if anchor_text.is_empty() {
                None
            } else {
                Some(anchor_text.to_string())
            };

            continue;
        }

        // Lines before the first @@ are not expected in a well-formed body.
        // Skip empty lines silently — they act as visual separators between hunks.
        if !in_hunk {
            if raw_line.trim().is_empty() {
                continue;
            }
            // Non-empty content before the first @@ is malformed.
            return Err(format!(
                "unexpected content before first @@ separator: {:?}",
                raw_line
            ));
        }

        // ── Inside a hunk: classify the line by its first character ────────

        if raw_line.starts_with('-') {
            // Removal line: strip the '-' prefix, add to old_lines only.
            let content = raw_line[1..].to_string();

            // Check for the EOF sentinel marker (case-insensitive trimmed check).
            if content.trim().eq_ignore_ascii_case("*** End of File") {
                // Mark the hunk as EOF-anchored and do NOT include the sentinel
                // in old_lines — it is a meta-marker, not real file content.
                is_end_of_file = true;
            } else {
                old_lines.push(content);
            }
        } else if raw_line.starts_with('+') {
            // Addition line: strip the '+' prefix, add to new_lines only.
            new_lines.push(raw_line[1..].to_string());
        } else if raw_line.starts_with(' ') {
            // Context line: strip the leading space, add to BOTH sides.
            // Context lines must be present in the file (used for seek) and
            // are preserved verbatim in the output.
            let content = raw_line[1..].to_string();
            old_lines.push(content.clone());
            new_lines.push(content);
        } else if raw_line.trim().is_empty() {
            // Empty lines between diff lines are treated as visual separators.
            // Skip them silently — they do not contribute to any hunk.
            continue;
        } else if raw_line.trim().eq_ignore_ascii_case("@@ EOF") {
            // "@@ EOF" as its own line (alternative EOF sentinel syntax).
            is_end_of_file = true;
        } else {
            // Any other content is a parse error — the model produced a line
            // that does not conform to the expected prefix rules.
            return Err(format!(
                "unexpected line in diff body (must start with '-', '+', ' ', or '@@'): {:?}",
                raw_line
            ));
        }
    }

    // ── Flush the last open hunk ───────────────────────────────────────────
    if in_hunk && (!old_lines.is_empty() || !new_lines.is_empty()) {
        hunks.push(EditHunk {
            seek_context,
            old_lines,
            new_lines,
            is_end_of_file,
        });
    }

    // ── Validate: at least one hunk must have been parsed ─────────────────
    if hunks.is_empty() {
        return Err(
            "diff body contained no valid hunks — provide at least one @@ block with diff lines"
                .to_string(),
        );
    }

    Ok(hunks)
}
