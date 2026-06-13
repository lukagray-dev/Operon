//! # operon-tools-web-fetch
//!
//! Implements the `web_fetch` tool for the Operon agent's web group.
//!
//! Fetches a URL and returns the page content as clean markdown. Strips navigation,
//! ads, and boilerplate. Supports:
//! - HTTP and HTTPS URLs
//! - HTML→markdown conversion via htmd
//! - Title extraction from <title> tag
//! - Content truncation at 10,000 characters
//! - HTTP error status codes (4xx, 5xx) returned as plain-text output, not errors
//! - Network-level failures return is_error: true
//!
//! ## Call format
//!
//! ```text
//! <web_fetch url="https://example.com">
//! ```
//!
//! ## Output format
//!
//! ```text
//! https://example.com
//! status: 200
//! title: Example Domain
//!
//! # Example Domain
//!
//! This domain is for use in illustrative examples...
//! ```
//!
//! If truncated:
//! ```text
//! [truncated — 15000 characters total, showing first 10000]
//! ```
//!
//! Non-2xx status:
//! ```text
//! https://example.com/missing
//! status: 404
//!
//! (no content — non-success status)
//! ```

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::WebFetchArgs;
pub use error::WebFetchToolError;

use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `web_fetch` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (URL scheme, content cap).
/// - `detailed`: sent after a malformed call. Full explanation with call format,
///   output format, error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "web_fetch".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses `args_json` and executes the web_fetch tool.
///
/// Returns a `ToolResult` with plain-text content (ToolContent::Text) on both
/// success and failure. Returns `Err(WebFetchToolError::ArgsParse)` if the
/// required `url` attribute is missing. Other validation failures (empty or
/// invalid scheme) return Ok with `is_error: true`.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON attr map produced by the dispatcher.
///
/// # Returns
/// - `Ok(ToolResult)` with plain-text content on either success or failure.
/// - `Err(WebFetchToolError::ArgsParse(reason))` if arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, WebFetchToolError> {
    // Parse the arguments — on failure, return an ArgsParse error so the dispatcher
    // can send the detailed tool definition back to the model, unless it is a
    // soft validation error (empty url or invalid scheme).
    let args = match WebFetchArgs::parse(&args_json) {
        Ok(a) => a,
        Err(e) => {
            if e.contains("missing") {
                return Err(WebFetchToolError::ArgsParse(e));
            }
            return Ok(ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(e),
                is_error: true,
                read_paths: None,
            });
        }
    };

    // Execute the fetch and return the result. The executor always returns a
    // ToolResult (never panics or propagates an error up).
    Ok(executor::execute(call_id, args).await)
}

/// Parses `args_json` and executes the web_fetch tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, WebFetchToolError> {
    // Parse the arguments first — fail fast before emitting any progress.
    let args = match WebFetchArgs::parse(&args_json) {
        Ok(a) => a,
        Err(e) => {
            if e.contains("missing") {
                return Err(WebFetchToolError::ArgsParse(e));
            }
            return Ok(ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(e),
                is_error: true,
                read_paths: None,
            });
        }
    };

    // Emit a progress event so the UI can show the URL being fetched.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "web_fetch",
            Some(args.url.clone()),
            format!("Fetching {}", args.url),
        ),
    );

    Ok(executor::execute(call_id, args).await)
}
