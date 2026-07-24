// =============================================================================
// Markdown Blockquote Formatter (`src/markdown/blockquote.rs`)
// =============================================================================
// Helper functions for formatting blockquote Markdown text.

/// Cleans raw blockquote text by trimming excess whitespace.
pub fn clean_blockquote_text(text: &str) -> String {
    text.trim().to_string()
}
