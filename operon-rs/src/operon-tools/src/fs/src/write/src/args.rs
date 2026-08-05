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
    /// The parent directory must already exist — this tool does not create
    /// intermediate directories. If the parent doesn't exist, the tool returns
    /// an error and the file is not modified.
    pub path: String,

    /// Full content to write to the file.
    /// For text files, this is the complete file content as a UTF-8 string.
    /// Existing content is completely replaced — this tool does not append or merge.
    /// The content is written atomically: if the write fails mid-operation, the
    /// original file (if it existed) is untouched.
    pub content: String,
}
