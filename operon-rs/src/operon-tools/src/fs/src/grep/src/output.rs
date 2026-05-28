/// Output types for the grep tool.
///
/// This module defines the structured result format returned by the grep tool.
/// Each file search produces a FileGrepResult, and all results are wrapped in
/// a GrepOutput container with summary statistics.

use serde::{Deserialize, Serialize};

/// A single line match (or context line) within a file.
///
/// Each line in the output is either a matching line (where the regex pattern was found)
/// or a context line (surrounding lines included via the `context_lines` parameter).
#[derive(Debug, Serialize, Deserialize)]
pub struct GrepLine {
    /// Line number in the file (1-indexed).
    ///
    /// This is the actual line number in the source file, not a relative index
    /// within the match results.
    pub line_no: usize,

    /// Line text content, with trailing newline removed.
    ///
    /// The content is the raw line from the file with `\r\n` or `\n` stripped.
    /// Leading/trailing whitespace within the line is preserved.
    pub content: String,

    /// Whether this line is a match or a context line.
    ///
    /// - `true`: This line contains the regex pattern (actual match)
    /// - `false`: This is a context line (included via `context_lines` parameter)
    pub is_match: bool,
}

/// All matches found in a single file.
///
/// Each file that was searched produces exactly one FileGrepResult, regardless
/// of whether matches were found or an error occurred.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileGrepResult {
    /// The path that was searched (echoed back for correlation).
    pub path: String,

    /// Total number of matching lines found in this file.
    ///
    /// This counts only actual matches (`is_match: true`), not context lines.
    /// If an error occurred, this will be 0.
    pub match_count: usize,

    /// All matching lines and their surrounding context lines.
    ///
    /// Lines are ordered by line number. Context lines from adjacent matches may
    /// overlap (the same line won't appear twice, but context ranges may merge).
    /// Empty if no matches were found or an error occurred.
    pub matches: Vec<GrepLine>,

    /// Human-readable error description if the file could not be searched.
    ///
    /// When populated, `match_count` will be 0 and `matches` will be empty.
    /// Examples:
    /// - "file too large, skipped (>10 MB)"
    /// - "binary file, skipped"
    /// - "failed to read file: Permission denied"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Top-level output returned to the model.
///
/// This is the complete result of a grep tool call, containing summary statistics
/// and per-file results.
#[derive(Debug, Serialize, Deserialize)]
pub struct GrepOutput {
    /// Total number of matching lines across all files.
    ///
    /// This is the sum of `match_count` from all FileGrepResult entries.
    /// Does not include context lines.
    pub total_matches: usize,

    /// Number of files that had at least one match.
    ///
    /// Files with errors or zero matches are not counted here.
    pub files_with_matches: usize,

    /// Whether the results were truncated due to hitting the match limit.
    ///
    /// When true, the search stopped early after reaching MAX_MATCHES (300).
    /// The current file being searched when the limit was hit is included in
    /// full, but subsequent files were not searched.
    pub truncated: bool,

    /// Per-file search results.
    ///
    /// Only includes files that had matches or errors. Files with zero matches
    /// and no errors are omitted to reduce output size.
    pub files: Vec<FileGrepResult>,
}
