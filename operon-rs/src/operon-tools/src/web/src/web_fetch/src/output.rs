//! Output types for the web_fetch tool.
//!
//! This module defines the structured result format returned by the web_fetch tool
//! on successful completion. Failures use ToolContent::Text directly — no struct needed.

use serde::{Deserialize, Serialize};

/// Output returned to the model on a successful fetch.
///
/// Returned even when the HTTP status is an error (4xx, 5xx) — the model receives
/// the status code and can decide whether to retry, try a different URL, or report
/// the error. Only network-level failures (can't connect, DNS failure, timeout) use
/// `ToolResult { is_error: true }`.
#[derive(Debug, Serialize, Deserialize)]
pub struct WebFetchOutput {
    /// The URL that was fetched (echoed back, may differ from input if redirected).
    /// Useful for correlation and debugging.
    pub url: String,

    /// HTTP status code.
    /// 200 = success, 404 = not found, 403 = forbidden, 500 = server error, etc.
    /// Non-2xx status codes are NOT tool errors — the model receives the status
    /// and can decide what to do next.
    pub status_code: u16,

    /// Page title extracted from <title> tag, if present.
    /// None if the page has no <title> tag or if the title is empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Page content as clean markdown, truncated to MAX_CONTENT_CHARS.
    /// Stripped of navigation, ads, and boilerplate by the htmd converter.
    /// Empty if the page is empty, the fetch failed, or the content is not HTML.
    pub content: String,

    /// True if content was truncated at MAX_CONTENT_CHARS.
    /// When true, the full page content is longer than 20,000 characters.
    /// Use a more specific URL or a more targeted query to get the relevant part.
    pub truncated: bool,

    /// Content length in characters (after truncation).
    /// Useful for understanding how much content was returned.
    pub content_length: usize,
}

impl WebFetchOutput {
    /// Formats the fetch output as raw plain text with section headers.
    pub fn to_plain_text(&self) -> String {
        if !(200..=299).contains(&self.status_code) {
            format!(
                "=== {} ({}) ===\nNo content (non-success status).",
                self.url, self.status_code
            )
        } else {
            let mut out = format!("=== {} ({}) ===\n", self.url, self.status_code);
            if let Some(ref title) = self.title {
                out.push_str(&format!("Title: {}\n", title));
            }
            out.push('\n');
            out.push_str(&self.content);
            if self.truncated {
                out.push_str(&format!("\n\n[truncated at {} chars]", self.content_length));
            }
            out
        }
    }
}
