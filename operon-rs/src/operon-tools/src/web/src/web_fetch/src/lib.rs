//! # operon-tools-web-fetch
//!
//! Implements the `web_fetch` tool for the Operon agent's web group.
//!
//! Fetches a URL and returns the page content as clean markdown. Strips navigation,
//! ads, and boilerplate. Supports:
//! - HTTP and HTTPS URLs
//! - Configurable timeout (default 15 seconds)
//! - HTML→markdown conversion via htmd
//! - Title extraction from <title> tag
//! - Content truncation at 20,000 characters
//! - HTTP error status codes (4xx, 5xx) returned as structured output, not errors
//! - Static content only (no JavaScript-rendered pages)
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_web_fetch::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "url": "https://www.rust-lang.org",
//!     "timeout_ms": 15000
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

pub use args::WebFetchArgs;
pub use error::WebFetchToolError;
pub use output::WebFetchOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use serde_json::json;

/// Returns the canonical tool definition for the `web_fetch` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`url`).
/// - Clear parameter descriptions for URL format and optional timeout.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the JSON Schema parameters for the web fetch tool here.
    let parameters = json!({
        "type": "object",
        "properties": {
            "url": {
                "type": "string",
                "description": "URL to fetch (must start with http:// or https://)."
            },
            "timeout_ms": {
                "type": "integer",
                "minimum": 1,
                "description": "Request timeout in milliseconds. Default: 15000."
            }
        },
        "required": ["url"]
    });

    ToolDefinition {
        name: "web_fetch".to_string(),
        description: "Fetches a URL and returns the page content as clean markdown. Pass `url` \
                      (http/https) and optionally `timeout_ms` (default: 15000). Content is stripped \
                      of navigation, ads, and boilerplate. Capped at 20,000 characters. Returns HTTP \
                      status code, page title, and markdown content."
            .to_string(),
        parameters,
    }
}

/// Deserializes `args_json` and executes the web_fetch tool.
///
/// Returns a `ToolResult` with either success (JSON WebFetchOutput) or failure (Text error message).
/// Returns `Err(WebFetchToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(WebFetchToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_web_fetch::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({
///         "url": "https://www.rust-lang.org",
///         "timeout_ms": 15000
///     })
/// ).await.unwrap();
/// assert_eq!(result.name, "web_fetch");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, WebFetchToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: WebFetchArgs = serde_json::from_value(args_json)?;

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}

/// Deserializes `args_json` and executes the web_fetch tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, WebFetchToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: WebFetchArgs = serde_json::from_value(args_json)?;

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
