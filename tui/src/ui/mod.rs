// UI module
// Root rendering entry point and all UI components
// Composes layout, chrome, screens, and widgets into the final TUI

pub mod chrome;
pub mod layout;
pub mod screens;
pub mod theme;
pub mod widgets;

use crate::state::AppState;
use ratatui::Frame;

/// Main render function
/// This is the single entry point called from main.rs event loop
/// Computes layout, renders chrome (sidebars, status bar), and active screen
pub fn render(frame: &mut Frame, state: &mut AppState) {
    // Compute layout areas based on current state
    let areas = layout::compute_layout(frame.area(), state);

    // Render left sidebar (file explorer) if open
    if state.is_left_sidebar_open() && areas.left_sidebar.width > 0 {
        chrome::left_sidebar::render(frame, areas.left_sidebar);
    }

    // Render main panel (active screen)
    screens::render_active_screen(frame, areas.main, state);

    // Render right sidebar (file preview, diff, terminal) if visible
    if state.right_panel().is_some() && areas.right_sidebar.width > 0 {
        chrome::right_sidebar::render(frame, areas.right_sidebar, state);
    }

    // Render status bar (always visible)
    chrome::status_bar::render_status_bar(frame, areas.status_bar, state);
}
