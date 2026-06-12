//! Executor for the web_fetch tool — handles all page scraping and HTML→markdown conversion.
//!
//! This module contains the core logic for fetching a single URL via the `spider`
//! crate, converting the HTML to clean markdown via `spider_transformations`, and
//! returning the result as plain text. The `chrome` feature of spider enables
//! headless Chrome support so JS-rendered pages are handled correctly.
//!
//! Architecture notes:
//! - `website.scrape()` with limit=1 fetches a single page without crawling further.
//! - `transform_content` handles HTML→markdown, stripping nav/ads/boilerplate.
//! - Content is truncated at MAX_CONTENT_CHARS to keep token usage reasonable.
//! - Non-2xx HTTP statuses → ToolContent::Text, is_error: false (informational).
//! - Network-level failures (no page returned) → ToolContent::Text, is_error: true.

use crate::args::WebFetchArgs;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use spider::website::Website;
use spider_transformations::transformation::content::{transform_content, TransformConfig};

/// Maximum content characters returned to the model.
/// Content is truncated at this limit to keep token usage reasonable.
/// Users can fetch more specific sub-pages to access the remaining content.
const MAX_CONTENT_CHARS: usize = 10_000;

/// Executes the web_fetch tool with the given arguments.
///
/// Scrapes the given URL with spider (limit=1, no crawling beyond the given URL),
/// converts HTML to markdown, and returns the result as plain text.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The parsed web_fetch arguments containing the URL.
///
/// # Returns
/// A `ToolResult` with plain-text content (ToolContent::Text) on both success and failure.
pub async fn execute(call_id: ToolCallId, args: WebFetchArgs) -> ToolResult {
    // Step 1: Build a single-page website crawler limited to exactly this URL.
    // limit(1) prevents following any links found on the page.
    // build() can fail on malformed URLs; fall back to the unbounded default if so.
    let mut website = Website::new(&args.url)
        .with_limit(1)
        .build()
        .unwrap_or_else(|_| Website::new(&args.url));

    // Step 2: Scrape the single page. This is the async entry point for spider.
    // spider handles its own timeouts and retries internally.
    website.scrape().await;

    // Step 3: Retrieve the scraped pages.
    // If spider returned no pages, it's a network-level failure (DNS, connection refused, etc.).
    let pages = match website.get_pages() {
        Some(p) if !p.is_empty() => p,
        _ => {
            return ToolResult {
                call_id,
                name: "web_fetch".to_string(),
                content: ToolContent::Text(format!(
                    "fetch failed: no page returned for {}",
                    args.url
                )),
                is_error: true,
                read_paths: None,
            };
        }
    };

    // Step 4: Extract metadata from the first (and only) page.
    let page = &pages[0];
    let status_code = page.status_code;
    let final_url = page.get_url_final();
    let html = page.get_html();

    // Step 5: Handle non-2xx HTTP status codes.
    // These are informational — the model receives the status and can decide what to do.
    // Only network-level failures (step 3) produce is_error: true.
    let status_u16 = status_code.as_u16();
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

    // Step 6: Extract title from <title> tag before conversion.
    // This is engine-agnostic — works on the raw HTML string.
    let title = extract_title(&html);

    // Step 7: Convert HTML to clean markdown via spider_transformations.
    // transform_content strips navigation, ads, scripts, and boilerplate,
    // and converts headings, lists, links, and code blocks to markdown.
    // This is the main improvement over the old htmd approach — spider_transformations
    // is specifically designed to produce clean, readable content for LLM consumption.
    let config = TransformConfig::default();
    let markdown = transform_content(page, &config, &None, &None, &None);

    // Step 8: Truncate to MAX_CONTENT_CHARS and build output.
    let original_length = markdown.chars().count();
    let truncated = original_length > MAX_CONTENT_CHARS;
    let content: String = if truncated {
        markdown.chars().take(MAX_CONTENT_CHARS).collect()
    } else {
        markdown
    };

    // Step 9: Build the plain-text output block.
    // Format:
    //   {final_url}
    //   status: {status_code}
    //   title: {title or "(none)"}
    //
    //   {content}
    //
    // If truncated, append a note about the full length.
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
///
/// Returns None if not found or if the title is empty.
/// This is a simple char-by-char search — no full HTML parser needed.
/// Engine-agnostic: works on any raw HTML string regardless of how it was fetched.
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
/// Compatibility stub — kept so tests.rs (which calls this function directly)
/// compiles until it is rewritten. In production the executor uses
/// spider_transformations::transform_content for HTML→markdown conversion.
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
