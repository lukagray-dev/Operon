// Right sidebar module
// Renders file previews, diffs, or terminal based on RightPanelContent
// Hidden by default, shown when AppState.right_panel is Some

pub mod diff_view;
pub mod file_preview;
pub mod panel_state;
pub mod terminal;

use crate::state::AppState;
use panel_state::RightPanelContent;
use ratatui::{layout::Rect, Frame};

/// Render the right sidebar based on current panel content
/// If AppState.right_panel is None, this function should not be called
/// (layout.rs gives it zero width in that case)
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    // Get panel content from state
    // If None, render nothing (this shouldn't happen due to layout logic)
    let Some(content) = state.right_panel() else {
        return;
    };

    // Dispatch to appropriate renderer based on content type
    match content {
        RightPanelContent::FilePreview(path) => {
            file_preview::render_file_preview(frame, area, path);
        }
        RightPanelContent::Diff(diff_text) => {
            diff_view::render_diff(frame, area, diff_text);
        }
        RightPanelContent::Terminal => {
            terminal::render_terminal(frame, area);
        }
    }
}
