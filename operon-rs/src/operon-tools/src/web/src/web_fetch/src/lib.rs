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
use operon_tools_core::TieredToolDefinition;
use serde_json::json;

/// Returns the tiered tool definition for the `web_fetch` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (URL scheme, content cap).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "url": {
                "type": "string",
                "description": "URL to fetch. Must start with http:// or https://."
            },
            "timeout_ms": {
                "type": "integer",
                "minimum": 1,
                "description": "Request timeout in milliseconds. Default: 15000."
            }
        },
        "required": ["url"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "web_fetch".to_string(),
            description: "Fetches a URL and returns the page content as clean markdown. Pass `url` \
                          (http/https) and optionally `timeout_ms` (default: 15000). Content is stripped \
                          of navigation, ads, and boilerplate. Capped at 20,000 characters. Returns HTTP \
                          status code, page title, and markdown content."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "web_fetch".to_string(),
            description: "\
Fetches a URL and returns the page content as clean markdown. Strips navigation, ads, and boilerplate.

## Input shapes

`url` (required, string): URL to fetch. Must start with http:// or https://.
Relative URLs are not supported — provide the full URL.

`timeout_ms` (optional, integer, milliseconds): Optional timeout for the request. Default: 15000 (15 seconds).
Increase for slow sites. There is no maximum — the model is responsible for setting a reasonable value.

## Output shape

Returns a JSON object with:
- `url`: The URL that was fetched (echoed back, may differ from input if redirected).
- `status_code`: HTTP status code (200 = success, 404 = not found, 500 = server error, etc.).
- `title`: Page title extracted from <title> tag, if present. Null if not found.
- `content`: Page content as clean markdown, truncated to 20,000 characters.
- `truncated`: True if content was truncated at 20,000 characters.
- `content_length`: Content length in characters (after truncation).

## HTTP status codes

HTTP error statuses (4xx, 5xx) are NOT tool errors. The model receives the status code and can decide:
- 404 (Not Found): Try a different URL or search for the correct page.
- 403 (Forbidden): The page is blocked or requires authentication.
- 500 (Server Error): The server is down — retry later or try a different source.
- 200 (Success): The page was fetched successfully.

Only network-level failures (can't connect, DNS failure, timeout) use `is_error: true`.

## Content truncation

If `truncated: true`, the full page content is longer than 20,000 characters.
To get the relevant part:
- Use a more specific URL (e.g., fetch the docs page directly, not the homepage).
- Use web_search with a more targeted query to find a more specific page.
- Extract the section you need from the truncated content.

## Title extraction

The `title` field contains the content of the <title> tag, if present.
If the page has no <title> tag or the title is empty, `title` is null.

## HTML→markdown conversion

The content is converted from HTML to markdown using htmd, which:
- Strips navigation, ads, and boilerplate
- Converts headings, lists, links, code blocks, etc. to markdown
- Removes inline styles and scripts
- Preserves text content and structure

If conversion fails (rare), a fallback plain-text extraction is used.

## Limitations

- Static content only: JavaScript-rendered pages (SPAs, dynamic content) may return empty or partial content.
  The tool fetches the initial HTML — it does not execute JavaScript.
- No authentication: The tool does not support cookies, authentication headers, or login flows.
- No redirects beyond HTTP: The tool follows HTTP redirects but does not handle meta-refresh or JavaScript redirects.

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

Example:
- Wrong: fetch https://example.com (homepage, 20,000 char limit)
- Right: fetch https://example.com/docs/api-reference (specific page)

### Mistake #2: Expecting JavaScript-rendered content
The tool fetches static HTML only. If a page is a single-page app (SPA) or heavily
JavaScript-dependent, the content may be empty or incomplete.

Example:
- Wrong: fetch a React SPA that renders content via JavaScript
- Right: fetch a static HTML page or use web_search to find a static docs page

### Mistake #3: Not checking the status code
If `status_code` is not 200, the content may be empty or an error page.
Always check the status code before processing the content.

### Mistake #4: Ignoring truncation
If `truncated: true`, you're only seeing the first 20,000 characters.
Use a more specific URL or a more targeted search to get the relevant part.

## Error messages

- \"url is empty\" → Provide a non-empty URL.
- \"url must start with http:// or https://\" → Use http:// or https://, not ftp:// or other schemes.
- \"fetch failed: ...\" → Network error (DNS failure, connection refused, timeout, etc.). Retry or try a different URL.
- \"failed to read response body: ...\" → The server sent an invalid response. Retry or try a different URL."
                .to_string(),
            parameters,
        },
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
