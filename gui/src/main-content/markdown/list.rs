// =============================================================================
// Markdown List Parser (`src/markdown/list.rs`)
// =============================================================================
// Helper functions for parsing list items, ordered list prefix formatting, and depth calculations.

/// Formats an ordered list prefix based on item index (1-based).
/// Preserves decimal numbers (1., 2., 3...) across all depths as written in the markdown document.
pub fn format_ordered_prefix(index: i32, _depth: i32) -> String {
    format!("{}.", index)
}
