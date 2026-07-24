// =============================================================================
// Markdown Paragraph Parser (`src/markdown/paragraph.rs`)
// =============================================================================
// Helper functions for parsing paragraph AST events from pulldown-cmark.

/// Sanitizes or formats paragraph text string.
pub fn clean_paragraph_text(text: &str) -> String {
    text.to_string()
}
