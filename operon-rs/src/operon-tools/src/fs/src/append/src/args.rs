//! Argument types for the append tool.
//!
//! This module defines the deserialization schema for the append tool's input.
//! The tool accepts a path and text content to append to an existing file.

use serde::Deserialize;

/// Arguments for the append tool.
///
/// Specifies a file path and the text content to append to that file.
/// The file must already exist — this tool does not create new files.
/// Use the write tool to create new files.
#[derive(Debug, Deserialize)]
pub struct AppendArgs {
    /// Absolute path to the file to append to.
    /// The file must already exist — use the write tool to create new files.
    /// If the file does not exist, the tool returns an error and nothing is appended.
    pub path: String,

    /// Text to append to the end of the file.
    /// Appended as-is — if a trailing newline separator is needed before the
    /// new content, include it at the start of this string.
    /// The content must be non-empty — appending empty content is an error.
    pub content: String,
}
