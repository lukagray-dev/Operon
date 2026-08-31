//! Argument types for the grep tool.
//!
//! Hey friend! This module defines the defensive deserialization schema for the grep tool's input,
//! supporting regex patterns, flexible path arrays (including stringified JSON),
//! glob filtering, and comprehensive parameter aliases.

use operon_tools_core::de::deserialize_flexible_string_list_opt;
use serde::Deserialize;

/// Top-level args the model sends when calling the `grep` tool.
#[derive(Debug, Deserialize)]
pub struct GrepArgs {
    /// Regex pattern to search for.
    #[serde(alias = "query", alias = "regex", alias = "search", alias = "text")]
    pub pattern: String,

    /// Target path(s) to search. Accepts a single string path, an array of paths,
    /// or a stringified JSON array.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_string_list_opt",
        alias = "paths",
        alias = "file_path",
        alias = "filePath",
        alias = "directory",
        alias = "dir",
        alias = "folder",
        alias = "target"
    )]
    pub path: Option<Vec<String>>,

    /// Optional glob pattern to filter files by name (e.g. "*.rs").
    #[serde(
        default,
        alias = "glob",
        alias = "filter",
        alias = "mask",
        alias = "file_pattern",
        alias = "filePattern"
    )]
    pub include: Option<String>,

    /// Case-insensitive matching. Default: false.
    #[serde(
        default,
        alias = "caseInsensitive",
        alias = "ignore_case",
        alias = "ignoreCase"
    )]
    pub case_insensitive: Option<bool>,

    /// Number of context lines before and after matches. Default: 2.
    #[serde(
        default = "default_context_lines",
        alias = "context",
        alias = "contextLines",
        deserialize_with = "deserialize_context_lines"
    )]
    pub context_lines: usize,
}

fn default_context_lines() -> usize {
    2
}

fn deserialize_context_lines<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = operon_tools_core::de::deserialize_flexible_usize_opt(deserializer)?;
    Ok(opt.unwrap_or(2))
}

impl GrepArgs {
    /// Returns the target paths as a vector.
    pub fn get_paths(&self) -> Vec<String> {
        self.path.clone().unwrap_or_default()
    }
}
