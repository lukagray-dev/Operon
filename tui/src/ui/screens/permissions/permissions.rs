// Permissions screen
// Owner vs External access control rules
// For bootstrap: renders placeholder content

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_TITLE};

/// Render the permissions configuration screen
/// For bootstrap: displays placeholder content
/// Future: Implement permission rule editor:
/// - Table of rules (tool, directory, owner/external access)
/// - Add/edit/delete rule buttons
/// - Per-tool permission toggles
/// - Per-directory access control
/// - Per-channel permission overrides
pub fn render_permissions_screen(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Permissions");

    let lines = vec![
        Line::from(Span::styled("Access Control", STYLE_TITLE)),
        Line::from(""),
        Line::from(Span::styled("Coming soon:", STYLE_MUTED)),
        Line::from(Span::styled("- Owner vs External user separation", STYLE_MUTED)),
        Line::from(Span::styled("- Per-tool permission rules", STYLE_MUTED)),
        Line::from(Span::styled("- Per-directory access control", STYLE_MUTED)),
        Line::from(Span::styled("- Per-channel permission overrides", STYLE_MUTED)),
        Line::from(""),
        Line::from(Span::styled("Define who can access what tools and directories.", STYLE_MUTED)),
    ];

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}
