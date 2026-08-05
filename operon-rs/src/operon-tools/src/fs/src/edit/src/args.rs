//! Argument types for the edit tool.
//!
//! This module defines the deserialization schema for the edit tool's input.
//! The tool accepts a path and an array of one or more search-and-replace hunks.

use serde::Deserialize;

/// A single search-and-replace operation.
///
/// Each hunk specifies an exact string to find and its replacement.
/// The old_string must match exactly once in the file (zero or multiple matches
/// are errors). The new_string must differ from old_string.
#[derive(Debug, Deserialize)]
pub struct EditHunk {
    /// Exact text to find in the file. Must match exactly once.
    /// Whitespace and indentation must be exact — copy from read output.
    /// The line number prefix from read output (e.g., "  123 | ") is display-only
    /// and must NOT be included in old_string.
    pub old_string: String,

    /// Replacement text. Must differ from old_string.
    /// If old_string and new_string are identical, the tool returns an error.
    pub new_string: String,
}

/// Arguments for the edit tool.
///
/// Specifies a file path and one or more edits to apply atomically.
#[derive(Debug, Deserialize)]
pub struct EditArgs {
    /// Absolute path to the file to edit.
    /// Also accepted as "file_path" for compatibility.
    #[serde(alias = "file_path")]
    pub path: String,

    /// One or more edits to apply. Applied in order on the in-memory content.
    /// All edits are committed in a single atomic write.
    /// Minimum 1 item — empty array is an error.
    pub edits: Vec<EditHunk>,
}
