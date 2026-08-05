//! Argument types for the ls tool.
//!
//! This module defines the deserialization schema for the ls tool's input.
//! The tool accepts a directory path and optional glob patterns to exclude.

use serde::Deserialize;

/// Arguments for the ls tool.
///
/// Accepts a directory path and optional glob patterns to exclude from results.
/// The `ignore` patterns are matched against entry names only (not full paths).
#[derive(Debug, Deserialize)]
pub struct LsArgs {
    /// Absolute path to the directory to list.
    /// Must be a directory. Listing a file path returns an error result (not Err).
    #[serde(alias = "dir")]
    pub path: String,

    /// Optional list of glob patterns to exclude from results.
    /// Matched against the entry name only (not the full path).
    /// Examples: ["*.lock", "node_modules", ".git", "target"]
    /// Default: empty (no exclusions).
    #[serde(default)]
    pub ignore: Option<Vec<String>>,
}
