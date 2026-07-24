// =============================================================================
// Markdown Italic Formatter (`src/markdown/italic.rs`)
// =============================================================================
// Returns Markdown italic delimiters used to reconstruct italic spans for
// slint::StyledText::from_markdown() native rendering.

/// Returns opening Markdown delimiter for italic text.
pub fn open_italic_tag() -> &'static str {
    "*"
}

/// Returns closing Markdown delimiter for italic text.
pub fn close_italic_tag() -> &'static str {
    "*"
}
