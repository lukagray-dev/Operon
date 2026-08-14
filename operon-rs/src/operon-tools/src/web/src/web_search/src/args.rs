//! Argument types for the web_search tool.
//!
//! Hey friend! Defines the defensive deserialization schema for the web_search tool's input.
//! Supports common query aliases, limit synonyms, and numeric string parsing.

use operon_tools_core::de::deserialize_flexible_usize_opt;
use serde::Deserialize;

/// Arguments for the web_search tool.
///
/// Specifies a search query and an optional maximum number of results to return.
/// The query supports DuckDuckGo syntax (site:, filetype:, quotes, etc.).
#[derive(Debug, Deserialize)]
pub struct WebSearchArgs {
    /// The search query. Same syntax as typing into DuckDuckGo.
    /// Supports advanced operators: site:, filetype:, quotes, etc.
    #[serde(
        alias = "q",
        alias = "search_query",
        alias = "searchQuery",
        alias = "search",
        alias = "text"
    )]
    pub query: String,

    /// Maximum number of results to return. Default: 5. Maximum: 10.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_usize_opt",
        alias = "limit",
        alias = "maxResults",
        alias = "count",
        alias = "n"
    )]
    pub max_results: Option<usize>,
}
