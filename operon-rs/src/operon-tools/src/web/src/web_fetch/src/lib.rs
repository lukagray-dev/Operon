//! # operon-tools-web-fetch
//!
//! Implements the `web_fetch` tool for the Operon agent's web group.
//!
//! Fetches a URL and returns the page content as clean markdown. Strips navigation,
//! ads, and boilerplate. Supports:
//! - HTTP and HTTPS URLs
//! - JS-rendered pages via headless Chrome (spider `chrome` feature)
//! - HTML→markdown conversion via spider_transformations
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
            description: "Fetches a URL and returns the page content as clean markdown. \
                          Call format: <web_fetch url=\"https://example.com\"> \
                          Supports JS-rendered pages via headless Chrome. \
                          Content is stripped of navigation, ads, and boilerplate. \
                          Capped at 10,000 characters. Returns status code, page title, and content."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "web_fetch".to_string(),
            description: "\
Fetches a URL and returns the page content as clean markdown. Strips navigation, ads, and boilerplate.
Supports JS-rendered pages via headless Chrome (spider chrome feature enabled).

## Call format

<web_fetch url=\"https://example.com\">

Single required attribute. The tool tag has no body. No timeout attr — spider
manages its own timeouts and retries internally.

## Attributes

`url` (required, string): URL to fetch. Must start with http:// or https://.
Relative URLs are not supported — provide the full absolute URL.

## Output format

Plain text output block:

  {final_url}
  status: {status_code}
  title: {title or \"(none)\"}

  {markdown content}

If the content was truncated:

  [truncated — {original_length} characters total, showing first 10000]

Non-2xx status (4xx, 5xx) — informational, NOT a tool error:

  {url}
  status: {status_code}

  (no content — non-success status)

## HTTP status codes

HTTP error statuses (4xx, 5xx) are NOT tool errors. The model receives the status and can decide:
- 404 (Not Found): Try a different URL or search for the correct page.
- 403 (Forbidden): The page is blocked or requires authentication.
- 500 (Server Error): The server is down — retry later or try a different source.
- 200 (Success): The page was fetched successfully.

Only network-level failures (can't connect, DNS failure, spider returned zero pages)
use `is_error: true` with a \"fetch failed: {reason}\" message.

## Content truncation

If `[truncated — ...]` appears at the end, the full page content is longer than 10,000 characters.
To get the relevant part:
- Use a more specific URL (e.g., fetch the docs page directly, not the homepage).
- Use web_search with a more targeted query to find a more specific page.
- Extract the section you need from the truncated content.

## JS-rendered pages

spider uses headless Chrome to execute JavaScript before returning the page HTML.
This means SPAs and dynamically-rendered pages are supported, unlike the previous
reqwest-based implementation which returned only the initial static HTML.

## HTML→markdown conversion

Content is converted to markdown via spider_transformations, which:
- Strips navigation, ads, scripts, and boilerplate
- Converts headings, lists, links, code blocks, etc. to markdown
- Removes inline styles
- Preserves text content and structure

## Common workflow

1. Use web_search to find relevant URLs.
2. Pick a promising result URL.
3. Use web_fetch to read the full content of that URL.
4. Extract the information you need from the fetched content.

## Common mistakes

### Mistake #1: Fetching a homepage when looking for specific docs
Homepages are often large and generic. If you're looking for specific documentation:
- Use web_search to find the specific docs page URL.
- Fetch that specific URL, not the homepage.

### Mistake #2: Not checking the status code
If `status: {N}` shows a non-200 code, the content field will be empty.
Always check the status line before processing content.

### Mistake #3: Ignoring truncation
If `[truncated ...]` appears, you're only seeing the first 10,000 characters.
Use a more specific URL or a more targeted search to get the relevant part.

## Error messages

- \"fetch failed: no page returned for {url}\" → Network error (DNS failure, connection refused, timeout, etc.). Retry or try a different URL.
- \"url is empty\" → Provide a non-empty URL.
- \"url must start with http:// or https://\" → Use http:// or https://, not ftp:// or other schemes."
                .to_string(),
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
