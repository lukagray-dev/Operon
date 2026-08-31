//! Argument types for the glob tool.
//!
//! Hey friend! This module defines the parameters the model sends to search for files
//! by glob pattern. It supports patterns like `**/*.rs`, `src/**/*.ts`, `*.json`, etc.

use serde::Deserialize;

/// Default maximum number of results to return.
pub const DEFAULT_MAX_RESULTS: usize = 100;
/// Hard ceiling on maximum results to prevent overwhelming the context window.
pub const MAX_RESULTS_LIMIT: usize = 1000;

/// Top-level arguments for the `glob` tool.
#[derive(Debug, Deserialize)]
pub struct GlobArgs {
    /// Glob pattern to match against relative file/dir paths (e.g. `**/*.rs`, `src/**/*.ts*`).
    #[serde(alias = "glob", alias = "query", alias = "file_pattern", alias = "pattern")]
    pub pattern: String,

    /// Base directory path to search within. Defaults to current directory if omitted.
    #[serde(
        default,
        alias = "paths",
        alias = "dir",
        alias = "directory",
        alias = "folder",
        alias = "base_path",
        alias = "file_path",
        alias = "filePath"
    )]
    pub path: Option<String>,

    /// Maximum number of matching paths to return. Default: 100, Max: 1000.
    #[serde(
        default = "default_max_results",
        alias = "limit",
        alias = "maxResults",
        deserialize_with = "deserialize_max_results"
    )]
    pub max_results: usize,

    /// Whether to include hidden files (e.g. dotfiles). Default: false.
    #[serde(default, alias = "hidden", alias = "includeHidden")]
    pub include_hidden: bool,
}

fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

fn deserialize_max_results<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = operon_tools_core::de::deserialize_flexible_usize_opt(deserializer)?;
    let val = opt.unwrap_or(DEFAULT_MAX_RESULTS);
    Ok(val.clamp(1, MAX_RESULTS_LIMIT))
}

