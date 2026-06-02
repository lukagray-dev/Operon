// File preview widget
// Displays read-only file content in the right panel
// For bootstrap: reads file and displays as plain text

use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_ERROR, STYLE_NORMAL};
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::path::Path;

/// Render a file preview in the right panel
/// Reads the file from disk and displays its content
/// For bootstrap: plain text only, no syntax highlighting
/// Future: Add syntax highlighting based on file extension
pub fn render_file_preview(frame: &mut Frame, area: Rect, file_path: &Path) {
    let title = format!(
        "Preview: {}",
        file_path.file_name().unwrap_or_default().to_string_lossy()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title(title);

    // Try to read file content
    let content = match std::fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => format!("Error reading file: {}", e),
    };

    // Determine style based on whether read was successful
    let style = if content.starts_with("Error") {
        STYLE_ERROR
    } else {
        STYLE_NORMAL
    };

    let paragraph = Paragraph::new(content)
        .block(block)
        .style(style)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
