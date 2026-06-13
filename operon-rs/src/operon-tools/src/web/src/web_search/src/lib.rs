//! # operon-tools-web-search
//!
//! Implements the `web_search` tool for the Operon agent's web group.
//!
//! Queries DuckDuckGo and returns plain-text search results (rank, title, URL, snippet).
//! No API key required. Supports:
//! - DuckDuckGo query syntax: site:, filetype:, quotes, etc.
//! - Configurable result count: 1–10 results (default 5)
//! - Plain-text output: one ranked block per result, joined by blank lines
//! - Empty results are valid (not an error)
//! - No JavaScript rendering (DuckDuckGo lite search, static content only)
//!
//! ## Call format
//!
//! ```text
//! <web_search query="rust async runtimes" max="10">
//! ```
//!
//! ## Output format
//!
//! ```text
//! 1. Page Title
//!    https://example.com/page
//!    Short snippet from the page content.
//!
//! 2. Another Title
//!    https://example.com/other
//!    Another snippet.
//! ```
//!
//! If no results are found:
//! ```text
//! No results for 'query'. Try different search terms.
//! ```

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::WebSearchArgs;
pub use error::WebSearchToolError;

use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `web_search` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (call format, result cap).
/// - `detailed`: sent after a malformed call. Full explanation with input attrs,
///   output format, error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "web_search".to_string(),
            description: "Searches DuckDuckGo and returns plain-text results. \
                          Call format: <web_search query=\"rust async runtimes\" max=\"10\"> \
                          `max` is optional (default 5, max 10). Returns each result as \
                          rank, title, URL, and snippet. No API key required. \
                          Use web_fetch to read the full content of any result URL."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "web_search".to_string(),
            description: "\
Searches DuckDuckGo and returns plain-text results (rank, title, URL, snippet). No API key required.

## Call format

<web_search query=\"rust async runtimes\" max=\"10\">

All attribute values are strings. The tool tag has no body.

## Attributes

`query` (required, string): Search query. Supports DuckDuckGo syntax:
- Exact phrase: \"machine learning\"
- Site search: site:github.com rust
- File type: filetype:pdf machine learning
- Exclude: -keyword
- Combine: site:github.com rust -deprecated

`max` (optional, string, represents integer 1–10): Number of results to return. Default: 5. Maximum: 10.
Capped at 10 — more results rarely improve agent outcomes and increase token usage significantly.

## Output format

Plain text. Each result block:

1. Page Title
   https://example.com/page
   Short snippet from the page content.

2. Another Title
   https://example.com/other
   Another snippet.

Results are separated by blank lines. Snippets are 100–200 characters.

## Empty results

If no results are found:
  No results for 'query'. Try different search terms.

This is NOT an error — the model receives the message and can decide how to proceed:
- Refine the query (fewer keywords, different terms)
- Use web_fetch directly if you have a specific URL

## Query syntax

Same as typing into DuckDuckGo:
- Quotes for exact phrases: \"exact phrase\"
- Site search: site:example.com
- File type: filetype:pdf
- Exclude: -keyword
- Combine operators: site:github.com rust -deprecated

## Common workflow

1. Use web_search to find relevant URLs.
2. Pick a promising result URL.
3. Use web_fetch to read the full content of that URL.
4. Extract the information you need from the fetched content.

## Common mistakes

### Mistake #1: Expecting full page content in snippet
Snippets are short (100–200 characters). If you need the full content, use web_fetch.

### Mistake #2: Requesting too many results
Requesting max=\"100\" will be capped at 10. Start with 5 and increase only if needed.

### Mistake #3: Not using DuckDuckGo syntax
Use site:, filetype:, quotes, and other operators for better results:
- site:github.com rust async
- \"machine learning\" -deprecated
- filetype:pdf neural networks

## Error messages

- \"No results for '...'\" → Refine the query or try different search terms.
- \"search failed: ...\" → Network error or DuckDuckGo API failure. Retry or try a different query."
                .to_string(),
        },
    }
}

/// Parses `args_json` and executes the web_search tool.
///
/// Returns a `ToolResult` with plain-text content (ToolContent::Text) on both
/// success and failure. Returns `Err(WebSearchToolError::ArgsParse)` only if the
/// required `query` attribute is missing. Other validation failures (e.g. empty
/// query) return Ok with `is_error: true`.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON attr map produced by the dispatcher.
///
/// # Returns
/// - `Ok(ToolResult)` with plain-text content on either success or failure.
/// - `Err(WebSearchToolError::ArgsParse(reason))` if arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, WebSearchToolError> {
    // Parse the arguments — on failure, return an ArgsParse error so the dispatcher
    // can send the detailed tool definition back to the model, unless it is a
    // soft validation error (empty query).
    let args = match WebSearchArgs::parse(&args_json) {
        Ok(a) => a,
        Err(e) => {
            if e.contains("missing") {
                return Err(WebSearchToolError::ArgsParse(e));
            }
            return Ok(ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text(e),
                is_error: true,
                read_paths: None,
            });
        }
    };

    // Execute the search and return the result. The executor always returns a
    // ToolResult (never panics or propagates an error up).
    Ok(executor::execute(call_id, args).await)
}

/// Parses `args_json` and executes the web_search tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, WebSearchToolError> {
    // Parse the arguments first — fail fast before emitting any progress.
    let args = match WebSearchArgs::parse(&args_json) {
        Ok(a) => a,
        Err(e) => {
            if e.contains("missing") {
                return Err(WebSearchToolError::ArgsParse(e));
            }
            return Ok(ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text(e),
                is_error: true,
                read_paths: None,
            });
        }
    };

    // Emit a progress event so the UI can show the query being searched.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "web_search",
            Some(args.query.clone()),
            format!("Searching DuckDuckGo for {}", args.query),
        ),
    );

    Ok(executor::execute(call_id, args).await)
}
