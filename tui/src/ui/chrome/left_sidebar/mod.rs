// Left sidebar module
// VS Code-style file explorer with directory tree
// For bootstrap: renders a placeholder

pub mod file_tree;
pub mod fs_state;

use ratatui::{layout::Rect, Frame};

/// Render the left sidebar (file explorer)
/// For bootstrap: displays a placeholder message
/// Future: Full file tree implementation with expand/collapse
pub fn render(frame: &mut Frame, area: Rect) {
    file_tree::render_file_tree(frame, area);
}
