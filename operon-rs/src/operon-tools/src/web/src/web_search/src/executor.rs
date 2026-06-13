//! Executor for the web_search tool — handles all DuckDuckGo search logic.
//!
//! This module contains the core logic for executing searches, parsing results,
//! formatting plain-text output, and handling errors. All DuckDuckGo I/O is
//! async via tokio. The spawn_blocking pattern is preserved unchanged because
//! the duckduckgo crate uses an embedded blocking runtime internally.

use crate::args::WebSearchArgs;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};


/// Default number of results to return if `max` is not specified.
const DEFAULT_RESULTS: usize = 5;

/// Maximum number of results to return, regardless of what the model requests.
/// Capped at 10 — more results rarely improve agent outcomes and increase token usage.
const MAX_RESULTS: usize = 10;

/// Executes the web_search tool with the given arguments.
///
/// Queries DuckDuckGo using the lite_search API (no JS rendering, static content only),
/// parses the results, and returns them as plain text. Each call is independent —
/// no state persists between calls.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The parsed web_search arguments containing the query and optional max.
///
/// # Returns
/// A `ToolResult` with plain-text content (ToolContent::Text) on both success and failure.
pub async fn execute(call_id: ToolCallId, args: WebSearchArgs) -> ToolResult {
    // Step 1: Cap `max` to the valid range [1, MAX_RESULTS].
    // Default to DEFAULT_RESULTS if not specified by the model.
    let max_results = args
        .max
        .unwrap_or(DEFAULT_RESULTS)
        .min(MAX_RESULTS)
        .max(1);

    // Step 2: Execute DuckDuckGo lite search inside spawn_blocking.
    // The duckduckgo crate uses an embedded blocking runtime internally — it MUST be
    // called from spawn_blocking, not directly from an async context.
    let query_owned = args.query.clone();
    let results_raw = tokio::task::spawn_blocking(move || {
        // Build a single-threaded tokio runtime for the blocking call.
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

    // Step 3: Handle spawn_blocking result.
    // Three cases: panic (task panicked), error (search failed), or success (got results).
    let raw_results = match results_raw {
        Err(_panic) => {
            return ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text("internal error: search task panicked".to_string()),
                is_error: true,
                read_paths: None,
            };
        }
        Ok(Err(e)) => {
            return ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text(e),
                is_error: true,
                read_paths: None,
            };
        }
        Ok(Ok(output)) => output,
    };

    // Step 4: Format results as plain text.
    // Each entry is:
    //   {rank}. {title}
    //      {url}
    //      {snippet}
    //
    // Entries are joined with a blank line.
    if raw_results.is_empty() {
        // No results found — guide the model to try different terms.
        return ToolResult {
            call_id,
            name: "web_search".to_string(),
            content: ToolContent::Text(format!(
                "No results for '{}'. Try different search terms.",
                args.query
            )),
            is_error: false,
            read_paths: None,
        };
    }

    // Build the plain-text result block, one entry per result.
    let text = raw_results
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            // rank is 1-indexed; indent URL and snippet by 3 spaces for readability.
            format!("{}. {}\n   {}\n   {}", i + 1, r.title, r.url, r.snippet)
        })
        .collect::<Vec<_>>()
        .join("\n\n"); // blank line between results

    ToolResult {
        call_id,
        name: "web_search".to_string(),
        content: ToolContent::Text(text),
        is_error: false,
        read_paths: None,
    }
}
