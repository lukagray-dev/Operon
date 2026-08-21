/// Output types for the read tool.
///
/// This module defines the structured result format returned by the read tool.
/// Each file read attempt produces a FileReadResult, and all results are wrapped
/// in a ReadOutput container.
use serde::{Deserialize, Serialize};

/// The outcome for a single file read attempt.
///
/// Each file in the read request produces exactly one FileReadResult, regardless
/// of whether the read succeeded or failed. Success/failure is indicated by the
/// `success` field, and error details are embedded in the `error` field.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileReadResult {
    /// The path that was requested (echoed back for correlation).
    pub path: String,

    /// `true` means `content` is populated and `error` is None.
    /// `false` means `error` is populated and `content` is None.
    pub success: bool,

    /// File contents on success. None on failure.
    ///
    /// For full-file reads, this is the entire file content (up to 1 MB limit).
    /// For line-range reads, this is the requested slice of lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Human-readable error description on failure. None on success.
    ///
    /// Examples:
    /// - "File not found"
    /// - "File exceeds 1 MB limit (2048576 bytes). Use start_line/end_line to read in chunks."
    /// - "Binary file detected. Use the image/video tool for media files."
    /// - "start_line 500 exceeds file length (100 lines)."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Total lines in the file (only populated on success, full-file reads).
    ///
    /// For line-range reads, this is also populated to give context about the
    /// full file size. For failures, this is None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_lines: Option<usize>,

    /// Actual lines returned (start..=end), 1-indexed. Only populated when a range was used.
    ///
    /// For full-file reads, this is None (the entire file was returned).
    /// For line-range reads, this shows the actual range that was returned after
    /// clamping to the file's line count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_returned: Option<LineRange>,
}

/// A 1-indexed, inclusive line range.
///
/// Used in FileReadResult to indicate which lines were actually returned when
/// a line range was requested.
#[derive(Debug, Serialize, Deserialize)]
pub struct LineRange {
    /// First line returned (1-indexed, inclusive).
    pub start: usize,
    /// Last line returned (1-indexed, inclusive).
    pub end: usize,
}

/// The complete output of a `read` tool call — one entry per requested path.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadOutput {
    /// Results for each file that was requested, in the same order as the input.
    pub files: Vec<FileReadResult>,
}

impl ReadOutput {
    /// Formats the read output as raw plain text with section headers.
    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for (i, file) in self.files.iter().enumerate() {
            if i > 0 {
                out.push_str("\n\n");
            }

            if file.success {
                let header = match (&file.lines_returned, file.total_lines) {
                    (Some(range), Some(total)) => {
                        format!(
                            "=== {} (lines {}-{} of {}) ===",
                            file.path, range.start, range.end, total
                        )
                    }
                    (Some(range), None) => {
                        format!(
                            "=== {} (lines {}-{}) ===",
                            file.path, range.start, range.end
                        )
                    }
                    (None, Some(total)) => {
                        format!("=== {} ({} lines) ===", file.path, total)
                    }
                    (None, None) => {
                        format!("=== {} ===", file.path)
                    }
                };
                out.push_str(&header);
                out.push('\n');
                if let Some(content) = &file.content {
                    out.push_str(content);
                }
            } else {
                out.push_str(&format!("=== {} ===\n", file.path));
                if let Some(err) = &file.error {
                    out.push_str(&format!("Error: {}", err));
                } else {
                    out.push_str("Error: Failed to read file");
                }
            }
        }
        out
    }
}
