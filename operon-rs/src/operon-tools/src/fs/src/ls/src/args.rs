//! Argument types for the ls tool.
//!
//! This module defines the deserialization schema for the ls tool's input.
//! The tool accepts a directory path and optional glob patterns to exclude.

use serde::Deserialize;

/// Arguments for the ls tool.
#[derive(Debug, Deserialize)]
pub struct LsArgs {
    /// Absolute or relative directory path to list. Defaults to "." if omitted.
    #[serde(alias = "dir", default = "default_path")]
    pub path: String,

    /// Optional list of glob patterns to exclude from results.
    #[serde(default)]
    pub ignore: Option<Vec<String>>,
}

fn default_path() -> String {
    ".".to_string()
}

