//! Argument types for the web_search tool.
//!
//! This module defines the deserialization schema for the web_search tool's input.
//! The tool accepts a search query and an optional maximum number of results.

use serde::Deserialize;

/// Arguments for the web_search tool.
///
/// Specifies a search query and an optional maximum number of results to return.
/// The query supports DuckDuckGo syntax (site:, filetype:, quotes, etc.).
#[derive(Debug, Deserialize)]
pub struct WebSearchArgs {
    /// The search query. Same syntax as typing into DuckDuckGo.
    /// Supports advanced operators: site:, filetype:, quotes, etc.
    pub query: String,

    /// Maximum number of results to return. Default: 5. Maximum: 10.
    /// Capped at 10 — more results rarely improve agent outcomes and
    /// increase token usage significantly.
    #[serde(default)]
    pub max_results: Option<usize>,
}
