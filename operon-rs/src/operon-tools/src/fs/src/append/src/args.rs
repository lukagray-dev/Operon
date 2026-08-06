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
    /// Absolute path to an existing file to append to.
    /// The file must already exist — use the write tool to create new files.
    pub path: String,

    /// Text content to append to the end of the file. Provide text naturally with normal \n line breaks.
    /// Appended as-is — if a separating newline is needed before the new content, include it at the start of this string.
    pub content: String,
}
