/// Internal output types for the read tool executor.
///
/// These types represent the per-file read result used internally by the executor
/// to assemble the final text output. They are NOT serialized to JSON — output
/// is formatted as plain text (ToolContent::Text).
///
/// All serde derives have been removed since these types are internal only.

/// The outcome for a single file read attempt.
///
/// Each file in the read request produces exactly one FileReadResult, regardless
/// of whether the read succeeded or failed. Success/failure is indicated by the
/// `success` field, and error details are embedded in the `error` field.
#[derive(Debug)]
pub struct FileReadResult {
    /// The path that was requested (echoed back for display in output).
    pub path: String,

    /// `true` means `content` is populated and `error` is None.
    /// `false` means `error` is populated and `content` is None.
    pub success: bool,

    /// File contents on success. None on failure.
    ///
    /// For full-file reads, this is the entire file content (up to 1 MB limit).
    /// For line-range reads, this is the requested slice of lines.
    pub content: Option<String>,

    /// Human-readable error description on failure. None on success.
    ///
    /// Examples:
    /// - "File not found"
    /// - "File exceeds 1 MB limit (2048576 bytes). Use a line range to read in chunks."
    /// - "Binary file detected. Use the image/video tool for media files."
    /// - "start_line 500 exceeds file length (100 lines)."
    pub error: Option<String>,

    /// Total lines in the file. Populated on success and on some error paths
    /// (e.g. start_line out of bounds still reports total_lines for context).
    pub total_lines: Option<usize>,

    /// Actual lines returned (start..=end), 1-indexed. Only populated when a range was used.
    ///
    /// For full-file reads, this is None (the entire file was returned).
    /// For line-range reads, this shows the actual range after clamping.
    pub lines_returned: Option<LineRange>,
}

/// A 1-indexed, inclusive line range.
///
/// Used in FileReadResult to track which lines were actually returned
/// when a line range was requested.
#[derive(Debug)]
pub struct LineRange {
    /// First line returned (1-indexed, inclusive).
    pub start: usize,
    /// Last line returned (1-indexed, inclusive).
    pub end: usize,
}
