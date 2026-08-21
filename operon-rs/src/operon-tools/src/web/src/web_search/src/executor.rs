//! Executor for the web_search tool — handles all DuckDuckGo search logic.
//!
//! This module contains the core logic for validating queries, executing searches,
//! parsing results, and handling errors. All DuckDuckGo I/O is async via tokio.

use crate::args::WebSearchArgs;
use crate::output::{SearchResult, WebSearchOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};

/// Default number of results to return if max_results is not specified.
const DEFAULT_RESULTS: usize = 5;

/// Maximum number of results to return, regardless of what the model requests.
/// Capped at 10 — more results rarely improve agent outcomes and increase token usage significantly.
const MAX_RESULTS: usize = 10;

/// Executes the web_search tool with the given arguments.
///
/// Queries DuckDuckGo using the lite_search API (no JS rendering, static content only),
/// parses the results into structured SearchResult objects, and returns them to the model.
/// Each call is independent — no state persists between calls.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized web_search arguments containing the query and optional max_results.
///
/// # Returns
/// A `ToolResult` with either success (Text formatted WebSearchOutput) or failure (Text error message).
pub async fn execute(call_id: ToolCallId, args: WebSearchArgs) -> ToolResult {
    // Step 1: Validate query is non-empty.
    // An empty query is a no-op and indicates a mistake by the model.
    let query = args.query.trim().to_string();
    if query.is_empty() {
        return ToolResult {
            call_id,
            name: "web_search".to_string(),
            content: ToolContent::Text("query is empty".to_string()),
            is_error: true,
        };
    }

    // Step 2: Cap max_results to the valid range [1, MAX_RESULTS].
    // Default to DEFAULT_RESULTS if not specified.
    let max_results = args
        .max_results
        .unwrap_or(DEFAULT_RESULTS)
        .clamp(1, MAX_RESULTS);

    // Step 3: Execute DuckDuckGo lite search inside spawn_blocking.
    // The duckduckgo crate uses an embedded blocking runtime internally — it MUST be
    // called from spawn_blocking, not from an async context directly.
    let query_owned = query.clone();
    let results_raw = tokio::task::spawn_blocking(move || {
        // Build a new tokio runtime for the blocking call.
        // The duckduckgo crate uses reqwest internally which needs a runtime.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to build runtime: {}", e))?;

        rt.block_on(async {
            use duckduckgo::browser::Browser;
            use duckduckgo::user_agents::get;

            let browser = Browser::new();
            let ua = get("firefox").unwrap_or_default();

            // Use lite_search which returns structured LiteSearchResult objects directly.
            // Parameters:
            // - query: the search query
            // - region: "wt-wt" for worldwide (no regional bias)
            // - max_results: maximum number of results to return
            // - user_agent: user agent string for the request
            browser
                .lite_search(
                    &query_owned,
                    "wt-wt", // worldwide region — no regional bias
                    Some(max_results),
                    ua,
                )
                .await
                .map_err(|e| format!("search failed: {}", e))
        })
    })
    .await;

    // Step 4: Handle spawn_blocking result.
    // Three cases: panic (task panicked), error (search failed), or success (got results).
    let raw_results = match results_raw {
        Err(_panic) => {
            return ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text("internal error: search task panicked".to_string()),
                is_error: true,
            };
        }
        Ok(Err(e)) => {
            return ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text(e),
                is_error: true,
            };
        }
        Ok(Ok(output)) => output,
    };

    // Step 5: Parse the raw LiteSearchResult objects into SearchResult structs.
    // LiteSearchResult has fields: title, url, snippet.
    // Map them directly with 1-indexed rank.
    let results: Vec<SearchResult> = raw_results
        .into_iter()
        .enumerate()
        .map(|(i, r)| SearchResult {
            rank: i + 1,
            title: r.title,
            url: r.url,
            snippet: r.snippet,
        })
        .collect();

    // Step 6: Return success.
    // Construct the output with the query, result count, and results.
    let output = WebSearchOutput {
        query,
        result_count: results.len(),
        results,
    };

    ToolResult {
        call_id,
        name: "web_search".to_string(),
        content: ToolContent::Text(output.to_plain_text()),
        is_error: false,
    }
}
