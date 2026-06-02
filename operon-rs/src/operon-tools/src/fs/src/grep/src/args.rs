/// Argument types for the grep tool.
///
/// This module defines the deserialization schema for the grep tool's input.
/// The tool accepts a regex pattern, a list of paths to search, and optional
/// filtering and context parameters.
use serde::Deserialize;

/// Top-level args the model sends when calling the `grep` tool.
///
/// The tool searches for a regex pattern across one or more files or directories.
/// Directories are walked recursively with gitignore rules respected by default.
#[derive(Debug, Deserialize)]
pub struct GrepArgs {
    /// Regex pattern to search for. Always treated as a regex (not a literal string).
    /// The pattern uses Rust regex syntax. Special characters must be escaped if
    /// searching for literal strings (e.g., `\\.` to match a literal dot).
    pub pattern: String,

    /// Files or directories to search. Each entry is a plain path string.
    /// Directories are walked recursively. Gitignore rules are respected.
    /// At least one path is required.
    ///
    /// Accepts both singular "path" and plural "paths" for flexibility.
    #[serde(alias = "path")]
    pub paths: Vec<String>,

    /// Optional glob pattern to filter files by name. Applied during directory
    /// walk. E.g. "*.rs" searches only Rust files. "*.{ts,tsx}" searches both.
    /// Has no effect when all entries in `paths` are direct files (not directories).
    ///
    /// Uses standard glob syntax:
    /// - `*` matches any sequence of characters within a path component
    /// - `?` matches any single character
    /// - `{a,b}` matches either `a` or `b`
    /// - `**` matches zero or more directories (e.g., `**/*.rs` matches all Rust files recursively)
    #[serde(default)]
    pub include: Option<String>,

    /// Case-insensitive matching. Default: false (case-sensitive).
    ///
    /// When true, the regex pattern matches regardless of case. For example,
    /// pattern "error" would match "Error", "ERROR", "error", etc.
    #[serde(default)]
    pub case_insensitive: Option<bool>,

    /// Number of context lines to include before and after each match.
    /// Same value applies to both before and after. Default: 0 (no context).
    ///
    /// Context lines are marked with `is_match: false` in the output to distinguish
    /// them from actual matching lines. Context lines from adjacent matches may overlap.
    #[serde(default)]
    pub context_lines: Option<usize>,
}
