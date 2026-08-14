//! Argument types for the edit tool.
//!
//! Hey friend! This module defines the deserialization schema for the edit tool's input.
//! The tool accepts a target file path and an array of one or more search-and-replace hunks.

use serde::{Deserialize, Serialize};

/// A single search-and-replace operation.
///
/// Each hunk specifies an exact (or fuzzy-matchable) string to find and its replacement.
/// The `old_string` must match uniquely in the file (zero matches or multiple ambiguous matches
/// are treated as errors for that specific hunk). The `new_string` must differ from `old_string`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditHunk {
    /// Exact text to find in the file. Must match uniquely.
    /// Whitespace and indentation should match the file content.
    /// Note: The line number prefix from read output (e.g., "  123 | ") is display-only
    /// and must NOT be included in `old_string`.
    pub old_string: String,

    /// Replacement text. Must differ from `old_string`.
    /// If `old_string` and `new_string` are identical, the call fails validation.
    pub new_string: String,
}

/// Arguments for the edit tool.
///
/// Specifies a target file path and one or more edits to apply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditArgs {
    /// Absolute path to the file to edit.
    /// Also accepted as "file_path" for compatibility.
    #[serde(alias = "file_path")]
    pub path: String,

    /// One or more edits to apply. Applied in order on the in-memory content.
    /// All successful edits are committed in a single atomic write.
    /// Minimum 1 item — an empty array is a fast-fail error.
    pub edits: Vec<EditHunk>,
}
