// =============================================================================
// Markdown Heading Parser (`src/markdown/heading.rs`)
// =============================================================================
// Helper functions for parsing heading AST events from pulldown-cmark.

use pulldown_cmark::HeadingLevel;

/// Converts a pulldown-cmark HeadingLevel into an integer (1 to 6).
pub fn heading_level_to_int(level: HeadingLevel) -> i32 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
