//! Executor for the web_fetch tool — handles web page scraping and HTML→markdown conversion.
//!
//! This module contains the core logic for fetching a single URL via reqwest
//! and converting the HTML to clean markdown via htmd.

use crate::args::WebFetchArgs;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};

/// Maximum content characters returned to the model.
/// Content is truncated at this limit to keep token usage reasonable.
const MAX_CONTENT_CHARS: usize = 10_000;

/// Executes the web_fetch tool with the given arguments.
///
/// Fetches the given URL, converts HTML to markdown, and returns the result as plain text.
pub async fn execute(call_id: ToolCallId, args: WebFetchArgs) -> ToolResult {
    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(format!("failed to initialize HTTP client: {}", e)),
                is_error: true,
                read_paths: None,
            };
        }
    };

    let response = match client.get(&args.url).send().await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(format!("fetch failed: {}", e)),
                is_error: true,
                read_paths: None,
            };
        }
    };

    let status_code = response.status();
    let final_url = response.url().to_string();
    let status_u16 = status_code.as_u16();

    // Handle non-2xx HTTP status codes.
    if !(200..300).contains(&status_u16) {
        return ToolResult {
            call_id,
            name: "web_fetch".to_string(),
            content: ToolContent::Text(format!(
                "{}\nstatus: {}\n\n(no content — non-success status)",
                final_url, status_u16
            )),
            is_error: false,
            read_paths: None,
        };
    }

    let html = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(format!("failed to read page content: {}", e)),
                is_error: true,
                read_paths: None,
            };
        }
    };

    // Extract title from <title> tag before conversion.
    let title = extract_title(&html);

    // Convert HTML to clean markdown via htmd.
    let converter = htmd::HtmlToMarkdown::default();
    let markdown = converter.convert(&html).unwrap_or_else(|_| html.clone());

    // Truncate to MAX_CONTENT_CHARS and build output.
    let original_length = markdown.chars().count();
    let truncated = original_length > MAX_CONTENT_CHARS;
    let content: String = if truncated {
        markdown.chars().take(MAX_CONTENT_CHARS).collect()
    } else {
        markdown
    };

    // Build the plain-text output block.
    let title_str = title
        .as_deref()
        .unwrap_or("(none)");

    let mut output_text = format!(
        "{}\nstatus: {}\ntitle: {}\n\n{}",
        final_url, status_u16, title_str, content
    );

    // Append truncation notice if the content was cut off.
    if truncated {
        output_text.push_str(&format!(
            "\n\n[truncated — {} characters total, showing first {}]",
            original_length, MAX_CONTENT_CHARS
        ));
    }

    ToolResult {
        call_id,
        name: "web_fetch".to_string(),
        content: ToolContent::Text(output_text),
        is_error: false,
        read_paths: None,
    }
}

/// Extracts the content of the <title> tag from raw HTML.
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
#[allow(dead_code)]
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
