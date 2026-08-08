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
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
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
Searches DuckDuckGo and returns results formatted as plain text.

## Input shapes

`query` (required, string): Search query (supports quotes, `site:`, `filetype:`, `-keyword`).
`max_results` (optional, integer, 1–10): Number of results to return (default: 5, max: 10).

## Response format

Returns plain text with query and numbered results:
Query: <query>
<result_count> result(s)

[1] <title>
    <url>
    <snippet>"
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

/// Deserializes `args_json` and executes the web_search tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, WebSearchToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: WebSearchArgs = serde_json::from_value(args_json)?;

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
