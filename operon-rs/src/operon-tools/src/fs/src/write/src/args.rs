//! Argument types for the write tool.
//!
//! Hey friend! This module defines the defensive deserialization schema for the write tool's input.
//! The tool accepts a file path and complete file content to write, supporting common LLM parameter aliases.

use serde::Deserialize;

/// Arguments for the write tool.
///
/// Specifies a file path and the complete content to write to that file.
/// The parent directory must already exist — this tool does not create
/// intermediate directories.
#[derive(Debug, Deserialize)]
pub struct WriteArgs {
    /// Absolute path to the file to create or overwrite.
    /// The parent directory must already exist.
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

    /// Complete text content to write to the file. Existing content is fully replaced.
    /// Provide the text naturally with standard \n line breaks.
    #[serde(
        alias = "text",
        alias = "body",
        alias = "__body__",
        alias = "file_content",
        alias = "fileContent",
        alias = "contents",
        alias = "data"
    )]
    pub content: String,
}
