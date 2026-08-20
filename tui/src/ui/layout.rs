// layout.rs — Master layout computation for Operon TUI.
//
// Defines the clean, full-width terminal layout:
// ┌──────────────────────────────────────────────┐
// │                                              │
// │                 Main Area                    │
// │              (Active Screen)                 │
// │                                              │
// ├──────────────────────────────────────────────┤
// │                Status Bar                    │
// └──────────────────────────────────────────────┘

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Computed layout areas for the major UI regions.
#[derive(Debug, Clone, Copy)]
pub struct LayoutAreas {
    /// Main content area (active screen).
    pub main: Rect,

    /// Persistent status bar area at the bottom.
    pub status_bar: Rect,
}

/// Compute layout areas for the given terminal frame size.
pub fn compute_layout(area: Rect) -> LayoutAreas {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Main area (active screen)
            Constraint::Length(1), // Status bar (exactly 1 line)
        ])
        .split(area);

    LayoutAreas {
        main: vertical_chunks[0],
        status_bar: vertical_chunks[1],
    }
}
