// =============================================================================
// Markdown Inline Code Formatter (`src/markdown/code.rs`)
// =============================================================================
// Helper functions for formatting inline code snippets using CommonMark backtick
// delimiters wrapped in HTML `<font color="...">` tags. The visual color palette
// is owned by `Tokens.code_text_color` in `ui/shared/tokens.slint`.

/// Default inline code text color hex matching `Tokens.code_text_color` (#79c0ff)
pub const DEFAULT_CODE_COLOR_HEX: &str = "#79c0ff";

/// Returns opening CommonMark backtick delimiter for inline code.
#[allow(dead_code)]
pub fn open_code_tag() -> &'static str {
    "`"
}

/// Returns closing CommonMark backtick delimiter for inline code.
#[allow(dead_code)]
pub fn close_code_tag() -> &'static str {
    "`"
}

/// Formats a raw inline code snippet with CommonMark backticks wrapped in a `<font color="...">`
/// tag driven by Slint's token theme color (`color_hex`).
pub fn format_inline_code_with_color(code: &str, color_hex: &str) -> String {
    if code.contains('`') {
        format!("<font color=\"{}\">`` {} ``</font>", color_hex, code)
    } else {
        format!("<font color=\"{}\">`{}`</font>", color_hex, code)
    }
}

/// Convenience formatter using the default Slint `Tokens.code_text_color` hex.
pub fn format_inline_code(code: &str) -> String {
    format_inline_code_with_color(code, DEFAULT_CODE_COLOR_HEX)
}
