// Notification widget
// Ephemeral toast-style status messages
// Appears at top-right corner and fades after a timeout

#![allow(dead_code)]

use ratatui::{
    layout::{Alignment, Rect},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use crate::ui::theme::{STYLE_ERROR, STYLE_NORMAL, STYLE_SUCCESS, STYLE_WARNING};

/// Notification severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Render a notification toast in the top-right corner
/// For bootstrap: basic implementation without fade animation
/// Future: Add fade-in/fade-out animation based on elapsed time
/// 
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The full screen area (notification will be positioned in top-right)
/// * `message` - Notification message text
/// * `level` - Severity level (determines styling)
pub fn render_notification(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    level: NotificationLevel,
) {
    // Calculate notification area (top-right corner, 30% width, 3 lines height)
    let width = (area.width * 30 / 100).max(20).min(50);
    let height = 3;
    
    let notification_area = Rect {
        x: area.width.saturating_sub(width + 1),
        y: 1,
        width,
        height,
    };

    // Clear the area behind the notification
    frame.render_widget(Clear, notification_area);

    // Choose style based on level
    let style = match level {
        NotificationLevel::Info => STYLE_NORMAL,
        NotificationLevel::Success => STYLE_SUCCESS,
        NotificationLevel::Warning => STYLE_WARNING,
        NotificationLevel::Error => STYLE_ERROR,
    };

    // Create notification block
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style);

    // Render message
    let paragraph = Paragraph::new(message)
        .block(block)
        .style(style)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, notification_area);
}
