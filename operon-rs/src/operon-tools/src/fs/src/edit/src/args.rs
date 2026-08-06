//! Argument types for the edit tool.
//!
//! Hey friend! This module defines the deserialization schema for the edit tool's input.
//! The tool accepts a file path and a unified-diff style patch string containing hunks.

use serde::Deserialize;

/// Arguments for the edit tool.
///
/// Specifies a target file path and a unified-diff style patch string to apply.
#[derive(Debug, Deserialize)]
pub struct EditArgs {
    /// Absolute path to the file to edit.
    /// Also accepted as "file_path" for compatibility.
    #[serde(alias = "file_path")]
    pub path: String,

    /// Unified-diff style patch body containing one or more @@ hunks.
    pub patch: String,
}
