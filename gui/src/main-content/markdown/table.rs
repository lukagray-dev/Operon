// =============================================================================
// Markdown Table Helpers (`src/markdown/table.rs`)
// =============================================================================
// Helper functions and enums for parsing CommonMark Markdown tables.

use pulldown_cmark::Alignment;

/// Converts pulldown_cmark `Alignment` enum to Slint integer representation:
/// 0 = Left / None, 1 = Center, 2 = Right
pub fn alignment_to_int(align: Alignment) -> i32 {
    match align {
        Alignment::Center => 1,
        Alignment::Right => 2,
        _ => 0,
    }
}
