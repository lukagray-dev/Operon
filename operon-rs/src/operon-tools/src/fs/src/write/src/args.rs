//! Argument types for the write tool.
//!
//! This module defines the deserialization schema for the write tool's input.
//! The tool accepts a path and complete file content to write.

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
    pub path: String,

    /// Complete text content to write to the file. Existing content is fully replaced.
    /// Provide the text naturally with standard \n line breaks.
    pub content: String,
}
