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
/// Computes layout, renders active screen in main area, and draws status bar at bottom
pub fn render(frame: &mut Frame, state: &mut AppState) {
    // Compute layout areas based on current terminal frame size
    let areas = layout::compute_layout(frame.area());

    // Render main panel (active screen)
    screens::render_active_screen(frame, areas.main, state);

    // Render status bar (always visible at bottom)
    chrome::status_bar::render_status_bar(frame, areas.status_bar, state);
}
