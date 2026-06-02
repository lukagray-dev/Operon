// Confirm dialog widget
// Modal yes/no prompt for destructive actions
// For bootstrap: basic implementation, will be enhanced later

use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_ERROR, STYLE_NORMAL};
#[allow(dead_code)]
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Render a confirmation dialog in the center of the screen
/// Used for destructive actions like deleting files, clearing history, etc.
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The full screen area (dialog will be centered)
/// * `title` - Dialog title
/// * `message` - Confirmation message to display
/// * `is_dangerous` - If true, uses error styling to indicate danger
pub fn render_confirm_dialog(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    message: &str,
    is_dangerous: bool,
) {
    // Calculate centered dialog area (50% width, 30% height)
    let dialog_area = centered_rect(50, 30, area);

    // Clear the area behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Create dialog block with appropriate styling
    let border_style = if is_dangerous {
        STYLE_ERROR
    } else {
        STYLE_ACTIVE_BORDER
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    // Split dialog into message area and button area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Message area
            Constraint::Length(3), // Button area
        ])
        .split(block.inner(dialog_area));

    // Render block
    frame.render_widget(block, dialog_area);

    // Render message
    let message_paragraph = Paragraph::new(message)
        .style(STYLE_NORMAL)
        .alignment(Alignment::Center);
    frame.render_widget(message_paragraph, chunks[0]);

    // Render button hints
    let button_text = "[Y]es  [N]o / [Esc]";
    let button_paragraph = Paragraph::new(button_text)
        .style(STYLE_NORMAL)
        .alignment(Alignment::Center);
    frame.render_widget(button_paragraph, chunks[1]);
}

/// Helper function to create a centered rectangle
/// Returns a Rect that is centered in the given area with specified percentage size
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
