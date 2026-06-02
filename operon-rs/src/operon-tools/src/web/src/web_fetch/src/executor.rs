//! Executor for the web_fetch tool — handles all HTTP fetching and HTML→markdown conversion.
//!
//! This module contains the core logic for validating URLs, building HTTP clients,
//! fetching content, extracting titles, converting HTML to markdown, and handling errors.
//! All HTTP I/O is async via reqwest.

use crate::args::WebFetchArgs;
use crate::output::WebFetchOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::time::Duration;

/// Default timeout in milliseconds if not specified.
const DEFAULT_TIMEOUT_MS: u64 = 15_000;

/// Maximum content characters returned to the model.
/// Content is truncated at this limit to keep token usage reasonable.
const MAX_CONTENT_CHARS: usize = 20_000;

/// Executes the web_fetch tool with the given arguments.
///
/// Fetches a URL via HTTP, extracts the title, converts HTML to markdown,
/// and returns the content to the model. Each call is independent — no state
/// persists between calls.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized web_fetch arguments containing the URL and optional timeout.
///
/// # Returns
/// A `ToolResult` with either success (JSON WebFetchOutput) or failure (Text error message).
pub async fn execute(call_id: ToolCallId, args: WebFetchArgs) -> ToolResult {
    // Step 1: Validate URL is non-empty and has a valid scheme.
    let url = args.url.trim().to_string();
    if url.is_empty() {
        return ToolResult {
            call_id,
            name: "web_fetch".to_string(),
            content: ToolContent::Text("url is empty".to_string()),
            is_error: true,
        };
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return ToolResult {
            call_id,
            name: "web_fetch".to_string(),
            content: ToolContent::Text("url must start with http:// or https://".to_string()),
            is_error: true,
        };
    }

    // Step 2: Build reqwest client with timeout.
    let timeout = Duration::from_millis(args.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));

    let client = match reqwest::Client::builder()
        .timeout(timeout)
        .user_agent("Mozilla/5.0 (compatible; Operon/1.0)")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(format!("failed to build HTTP client: {}", e)),
                is_error: true,
            };
        }
    };

    // Step 3: Fetch the URL.
    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(format!("fetch failed: {}", e)),
                is_error: true,
            };
        }
    };

    let status_code = response.status().as_u16();
    let final_url = response.url().to_string();

    // Step 4: For error status codes, return structured output (not is_error: true).
    // The model sees the status and can decide what to do next.
    if !response.status().is_success() {
        let output = WebFetchOutput {
            url: final_url,
            status_code,
            title: None,
            content: String::new(),
            truncated: false,
            content_length: 0,
        };
        return ToolResult {
            call_id,
            name: "web_fetch".to_string(),
            content: ToolContent::Json(
                serde_json::to_value(&output)
                    .unwrap_or_else(|_| serde_json::json!({ "status_code": status_code })),
            ),
            is_error: false,
        };
    }

    // Step 5: Read body as text.
    let html = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(format!("failed to read response body: {}", e)),
                is_error: true,
            };
        }
    };

    // Step 6: Extract title from HTML before converting.
    let title = extract_title(&html);

    // Step 7: Convert HTML to markdown with htmd.
    let markdown = match htmd::convert(&html) {
        Ok(md) => md,
        Err(_) => {
            // Fallback: strip all HTML tags with a simple char-by-char pass.
            // This is a best-effort fallback — htmd rarely fails.
            plain_text_fallback(&html)
        }
    };

    // Step 8: Truncate and return.
    let truncated = markdown.chars().count() > MAX_CONTENT_CHARS;
    let content: String = if truncated {
        markdown.chars().take(MAX_CONTENT_CHARS).collect()
    } else {
        markdown
    };
    let content_length = content.chars().count();

    let output = WebFetchOutput {
        url: final_url,
        status_code,
        title,
        content,
        truncated,
        content_length,
    };

    ToolResult {
        call_id,
        name: "web_fetch".to_string(),
        content: ToolContent::Json(serde_json::to_value(&output).unwrap_or_else(
            |e| serde_json::json!({ "error": format!("serialization bug: {}", e) }),
        )),
        is_error: false,
    }
}

/// Extracts the content of the <title> tag from raw HTML.
///
/// Returns None if not found or if the title is empty.
/// This is a simple char-by-char search — no full HTML parser needed.
pub(crate) fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower.find("</title>")?;
    if end <= start {
        return None;
    }
    let raw = &html[start..end];
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Strips HTML tags with a simple char-by-char pass.
///
/// Used only when htmd conversion fails — which is rare.
/// This is a best-effort fallback that removes all HTML tags and returns plain text.
pub(crate) fn plain_text_fallback(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}
