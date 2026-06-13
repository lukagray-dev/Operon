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
            description: include_str!("description.md").to_string(),
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
