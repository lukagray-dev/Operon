// File tree widget
// Recursive directory tree with expand/collapse functionality
// For bootstrap: renders a placeholder message

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::ui::theme::{STYLE_INACTIVE_BORDER, STYLE_MUTED};

/// Render the file tree in the left sidebar
/// For bootstrap: displays a placeholder message
/// Future: Implement recursive directory walking and tree rendering
/// - Use box-drawing characters for tree structure (├─, └─, │)
/// - Show expand/collapse indicators (▶, ▼)
/// - Highlight selected file
/// - Support keyboard navigation (up/down/enter)
pub fn render_file_tree(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_INACTIVE_BORDER)
        .title("Files");

    let placeholder_text = "File explorer\ncoming soon.\n\n\
                           Will show:\n\
                           - Directory tree\n\
                           - Expand/collapse\n\
                           - File selection\n\n\
                           Press Tab to\nswitch screens.";

    let paragraph = Paragraph::new(placeholder_text)
        .block(block)
        .style(STYLE_MUTED);

    frame.render_widget(paragraph, area);
}
