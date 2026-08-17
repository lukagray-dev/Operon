//! Argument types for the memory_search tool.

use serde::Deserialize;

/// Arguments for the memory_search tool.
#[derive(Debug, Deserialize)]
pub struct MemorySearchArgs {
    /// Full-text search query. FTS5 MATCH syntax is supported (AND, OR, NOT, phrase quotes).
    /// Must be non-empty after trimming.
    ///
    /// Aliases to handle model variation.
    #[serde(alias = "q", alias = "text", alias = "term", alias = "terms")]
    pub query: String,

    /// Maximum number of results to return, ranked by FTS5 relevance. Defaults to 10.
    #[serde(default)]
    pub limit: Option<usize>,
}
