// =============================================================================
// Markdown Line Spacer Helper (`src/markdown/spacer.rs`)
// =============================================================================
// Helper functions for computing vertical line break gaps between Markdown blocks.

/// Calculates the number of blank lines between two byte offsets in raw Markdown source text.
/// If there are `N` consecutive newlines (`\n`), there are `N - 1` blank lines.
pub fn calculate_blank_lines(slice: &str) -> i32 {
    let newlines = slice.chars().filter(|&c| c == '\n').count() as i32;
    (newlines - 1).max(0)
}
