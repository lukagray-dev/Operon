//! # operon-tools-web-search
//!
//! Implements the `web_search` tool for the Operon agent's web group.
//!
//! Queries DuckDuckGo and returns structured search results (title, URL, snippet).
//! No API key required. Supports:
//! - DuckDuckGo query syntax: site:, filetype:, quotes, etc.
//! - Configurable result count: 1–10 results (default 5)
//! - Structured output: rank, title, URL, snippet for each result
//! - Empty results are valid (not an error)
//! - Static content only (no JavaScript-rendered pages)
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_web_search::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "query": "rust programming language",
//!     "max_results": 5
//! });
//! let result = execute(
//!     ToolCallId("call_123".to_string()),
//!     args
//! ).await.unwrap();
//! # }
//! ```

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::WebSearchArgs;
pub use error::WebSearchToolError;
pub use output::{SearchResult, WebSearchOutput};

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::TieredToolDefinition;
use serde_json::json;

/// Returns the tiered tool definition for the `web_search` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (query syntax, result cap).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "Search query. Supports DuckDuckGo syntax: quotes, site:, filetype:, etc."
            },
            "max_results": {
                "type": "integer",
                "minimum": 1,
                "maximum": 10,
                "description": "Number of results to return. Default: 5. Maximum: 10."
            }
        },
        "required": ["query"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "web_search".to_string(),
            description: "Searches DuckDuckGo and returns structured results. Pass `query` (search string) \
                          and optionally `max_results` (1–10, default 5). Returns title, URL, and snippet \
                          for each result. No API key required. Use web_fetch to read the full content of \
                          any result URL."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "web_search".to_string(),
            description: "\
Searches DuckDuckGo and returns structured results (title, URL, snippet). No API key required.

## Input shapes

`query` (required, string): Search query. Supports DuckDuckGo syntax:
- Exact phrase: \"machine learning\"
- Site search: site:github.com rust
- File type: filetype:pdf machine learning
- Exclude: -keyword
- Combine: site:github.com rust -deprecated

`max_results` (optional, integer, 1–10): Number of results to return. Default: 5. Maximum: 10.
Capped at 10 — more results rarely improve agent outcomes and increase token usage significantly.

## Output shape

Returns a JSON object with:
- `query`: The query that was executed (echoed back).
- `result_count`: Number of results returned (0 if no results found).
- `results`: Array of search results, each with:
  - `rank`: Result rank, 1-indexed.
  - `title`: Page title.
  - `url`: Result URL.
  - `snippet`: Short description/snippet from the page.

## Result snippets

Snippets are short (typically 100–200 characters). They are NOT the full page content.
To read the full content of a result, use the `web_fetch` tool with the result's URL.

## Empty results

If no results are found, `result_count` is 0 and `results` is an empty array.
This is NOT an error — the model receives the empty results and can decide how to proceed:
- Refine the query (fewer keywords, different terms)
- Try a different search engine (web_search only uses DuckDuckGo)
- Use web_fetch directly if you have a specific URL

## Query syntax

Same as typing into DuckDuckGo:
- Quotes for exact phrases: \"exact phrase\"
- Site search: site:example.com
- File type: filetype:pdf
- Exclude: -keyword
- Combine operators: site:github.com rust -deprecated

## Limitations

- Static content only: JavaScript-rendered pages (SPAs, dynamic content) may return empty or partial snippets.
- No API key required: Uses DuckDuckGo's public lite search API.
- Privacy: DuckDuckGo does not track queries.

## Common workflow

1. Use web_search to find relevant URLs.
2. Pick a promising result URL.
3. Use web_fetch to read the full content of that URL.
4. Extract the information you need from the fetched content.

## Common mistakes

### Mistake #1: Expecting full page content in snippet
Snippets are short (100–200 characters). If you need the full content, use web_fetch.

### Mistake #2: Searching for JavaScript-rendered content
web_search returns static HTML only. If a page is a single-page app (SPA) or heavily
JavaScript-dependent, the snippet may be empty or incomplete. Try a different search
or use web_fetch on a more specific URL.

### Mistake #3: Not using DuckDuckGo syntax
You can use site:, filetype:, quotes, and other operators. Combine them for better results:
- site:github.com rust async
- \"machine learning\" -deprecated
- filetype:pdf neural networks

### Mistake #4: Requesting too many results
Requesting max_results: 100 will be capped at 10. More results rarely improve outcomes
and increase token usage. Start with 5 and increase only if needed.

## Error messages

- \"query is empty\" → Provide a non-empty query.
- \"search failed: ...\" → Network error or DuckDuckGo API failure. Retry or try a different query."
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the web_search tool.
///
/// Returns a `ToolResult` with either success (JSON WebSearchOutput) or failure (Text error message).
/// Returns `Err(WebSearchToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(WebSearchToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_web_search::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({
///         "query": "rust programming",
///         "max_results": 5
///     })
/// ).await.unwrap();
/// assert_eq!(result.name, "web_search");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, WebSearchToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: WebSearchArgs = serde_json::from_value(args_json)?;

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
