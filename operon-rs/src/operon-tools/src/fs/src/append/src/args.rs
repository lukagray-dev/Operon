//! Argument types for the append tool.
//!
//! Hey friend! This module defines the defensive deserialization schema for the append tool's input.
//! The tool accepts a path and text content to append to an existing file, supporting common LLM parameter aliases.

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
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "filepath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file",
        alias = "filename",
        alias = "fileName"
    )]
    pub path: String,

    /// Text content to append to the end of the file. Provide text naturally with normal \n line breaks.
    /// Appended as-is — if a separating newline is needed before the new content, include it at the start of this string.
    #[serde(
        alias = "text",
        alias = "body",
        alias = "__body__",
        alias = "append_text",
        alias = "appendText",
        alias = "data"
    )]
    pub content: String,
}
