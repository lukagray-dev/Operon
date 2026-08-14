//! Argument types for the ls tool.
//!
//! Hey friend! This module defines the defensive deserialization schema for the ls tool's input.
//! The tool accepts a directory path (defaulting to "." if omitted or passed as an empty string)
//! and optional glob patterns to exclude.

use operon_tools_core::de::{
    default_dot_path, deserialize_default_dir_path, deserialize_flexible_string_list_opt,
};
use serde::Deserialize;

/// Arguments for the ls tool.
#[derive(Debug, Deserialize)]
pub struct LsArgs {
    /// Absolute or relative directory path to list. Defaults to "." if omitted or empty.
    #[serde(
        default = "default_dot_path",
        deserialize_with = "deserialize_default_dir_path",
        alias = "dir",
        alias = "directory",
        alias = "folder",
        alias = "file_path",
        alias = "filePath",
        alias = "target"
    )]
    pub path: String,

    /// Optional list of glob patterns to exclude from results.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_string_list_opt",
        alias = "exclude",
        alias = "patterns"
    )]
    pub ignore: Option<Vec<String>>,
}
