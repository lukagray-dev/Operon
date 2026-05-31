//! Output types for the web_search tool.
//!
//! This module defines the structured result format returned by the web_search tool
//! on successful completion. Failures use ToolContent::Text directly — no struct needed.

use serde::{Deserialize, Serialize};

/// A single search result.
///
/// Represents one result from a DuckDuckGo search query.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// Result rank, 1-indexed.
    /// The first result has rank 1, the second has rank 2, etc.
    pub rank: usize,

    /// Page title.
    /// The title of the web page as returned by DuckDuckGo.
    pub title: String,

    /// Result URL.
    /// The full URL to the web page.
    pub url: String,

    /// Short description/snippet from the page.
    /// A brief excerpt from the page content. Use web_fetch to read the full content.
    pub snippet: String,
}

/// Top-level output returned to the model on successful search.
///
/// Returned even when no results are found — the model receives an empty results
/// array and can decide how to proceed (refine query, try different search terms, etc.).
#[derive(Debug, Serialize, Deserialize)]
pub struct WebSearchOutput {
    /// The query that was executed (echoed back).
    /// Useful for correlation and debugging.
    pub query: String,

    /// Number of results returned.
    /// Will be 0 if no results were found, up to max_results if results were found.
    pub result_count: usize,

    /// The search results.
    /// Each result contains rank, title, URL, and snippet.
    pub results: Vec<SearchResult>,
}
