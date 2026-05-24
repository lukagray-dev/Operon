// Diff viewer widget
// Renders unified diff format with syntax highlighting
// For bootstrap: renders raw diff string as-is

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_NORMAL};

/// Render a unified diff in the right panel
/// For bootstrap: displays raw diff string without parsing
/// Future: Parse diff format and apply line-by-line styling (green for +, red for -)
pub fn render_diff(frame: &mut Frame, area: Rect, diff_content: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Diff");

    let paragraph = Paragraph::new(diff_content)
        .block(block)
        .style(STYLE_NORMAL)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
