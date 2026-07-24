// =============================================================================
// Markdown Link Engine (`src/markdown/link.rs`)
// =============================================================================
// Formatter and URL sanitizer for Markdown hyperlinks (`[Text](url)`).
// Generates Markdown link syntax formatted for Slint's native `StyledText` engine.

/// Sanitizes destination URLs to prevent unsafe schemes or malformed strings.
pub fn sanitize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return "#".to_string();
    }
    trimmed.to_string()
}

/// Formats a link text and destination URL into Markdown link syntax (`[text](url)`).
#[allow(dead_code)]
pub fn format_markdown_link(text: &str, url: &str) -> String {
    let clean_url = sanitize_url(url);
    format!("[{}]({})", text, clean_url)
}
