// Embedded terminal widget
// Displays a pseudo-terminal for running commands
// For bootstrap: renders a placeholder block

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED};

/// Render an embedded terminal in the right panel
/// For bootstrap: displays a placeholder message
/// Future: Integrate with a real pseudo-terminal (pty) for command execution
pub fn render_terminal(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Terminal");

    let placeholder_text = "Terminal integration coming soon.\n\n\
                           This will allow you to:\n\
                           - Run shell commands\n\
                           - View command output\n\
                           - Interact with running processes\n\n\
                           Press Esc to close this panel.";

    let paragraph = Paragraph::new(placeholder_text)
        .block(block)
        .style(STYLE_MUTED);

    frame.render_widget(paragraph, area);
}
