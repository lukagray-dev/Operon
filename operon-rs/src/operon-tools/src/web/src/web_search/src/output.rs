//! Output types for the web_search tool.
//!
//! Output is now plain text built directly in executor.rs — no JSON structs needed.
//!
//! The types below are compatibility stubs kept only so existing tests.rs can
//! compile until tests are rewritten to match the new plain-text output format.
//! They will be removed when tests.rs is updated.

use serde::{Deserialize, Serialize};

/// A single search result (compatibility stub — output format is now plain text).
///
/// Kept only so tests.rs compiles until it is rewritten.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// Result rank, 1-indexed.
    pub rank: usize,
    /// Page title.
    pub title: String,
    /// Result URL.
    pub url: String,
    /// Short description/snippet from the page.
    pub snippet: String,
}

/// Top-level output (compatibility stub — output format is now plain text).
///
/// Kept only so tests.rs compiles until it is rewritten.
#[derive(Debug, Serialize, Deserialize)]
pub struct WebSearchOutput {
    /// The query that was executed (echoed back).
    pub query: String,
    /// Number of results returned.
    pub result_count: usize,
    /// The search results.
    pub results: Vec<SearchResult>,
}
