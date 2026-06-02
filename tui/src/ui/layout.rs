// Master layout computation
// Defines the overall TUI structure: left sidebar | main panel | right sidebar | status bar
// Right sidebar collapses to zero width when hidden

use crate::state::AppState;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Computed layout areas for all major UI regions
/// Returned by compute_layout() and used by render functions
#[derive(Debug, Clone, Copy)]
pub struct LayoutAreas {
    /// Left sidebar area (file explorer)
    pub left_sidebar: Rect,

    /// Main content area (active screen)
    pub main: Rect,

    /// Right sidebar area (file preview, diff, terminal)
    /// Zero width when hidden
    pub right_sidebar: Rect,

    /// Status bar area (bottom bar)
    pub status_bar: Rect,
}

/// Compute layout areas based on current application state
/// Layout structure:
/// ```
/// ┌─────────────┬──────────────────┬─────────────┐
/// │             │                  │             │
/// │    Left     │       Main       │    Right    │
/// │   Sidebar   │      Panel       │   Sidebar   │
/// │             │                  │  (optional) │
/// │             │                  │             │
/// ├─────────────┴──────────────────┴─────────────┤
/// │                Status Bar                    │
/// └──────────────────────────────────────────────┘
/// ```
///
/// Left sidebar: 20% width (or 0 if collapsed)
/// Right sidebar: 30% width (or 0 if hidden)
/// Main panel: Remaining width
/// Status bar: 1 line height at bottom
pub fn compute_layout(area: Rect, state: &AppState) -> LayoutAreas {
    // Split into main area and status bar
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Main area (at least 3 lines)
            Constraint::Length(1), // Status bar (exactly 1 line)
        ])
        .split(area);

    let main_area = vertical_chunks[0];
    let status_bar = vertical_chunks[1];

    // Compute horizontal layout: left sidebar | main | right sidebar
    let left_width = if state.is_left_sidebar_open() {
        20 // 20% of width
    } else {
        0 // Collapsed
    };

    let right_width = if state.right_panel().is_some() {
        30 // 30% of width
    } else {
        0 // Hidden
    };

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_width),
            Constraint::Min(20), // Main panel (at least 20 columns)
            Constraint::Percentage(right_width),
        ])
        .split(main_area);

    LayoutAreas {
        left_sidebar: horizontal_chunks[0],
        main: horizontal_chunks[1],
        right_sidebar: horizontal_chunks[2],
        status_bar,
    }
}
