// =============================================================================
// Markdown Bold Formatter (`src/markdown/bold.rs`)
// =============================================================================
// Returns Markdown bold delimiters used to reconstruct bold spans for
// slint::StyledText::from_markdown() native rendering.

/// Returns opening Markdown delimiter for bold text.
pub fn open_bold_tag() -> &'static str {
    "**"
}

/// Returns closing Markdown delimiter for bold text.
pub fn close_bold_tag() -> &'static str {
    "**"
}
