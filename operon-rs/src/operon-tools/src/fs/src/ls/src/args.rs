//! Argument types for the ls tool.
//!
//! Hey friend! This module defines the defensive deserialization schema for the ls tool's input.
//! The tool requires an absolute directory path and optional glob patterns to exclude.
//! Operon strictly requires absolute paths across all filesystem tools so that tools remain
//! stateless and deterministic without depending on process-wide current working directory state.

use operon_tools_core::de::deserialize_flexible_string_list_opt;
use serde::Deserialize;

/// Arguments for the ls tool.
#[derive(Debug, Deserialize)]
pub struct LsArgs {
    /// Absolute directory path to list.
    #[serde(
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
