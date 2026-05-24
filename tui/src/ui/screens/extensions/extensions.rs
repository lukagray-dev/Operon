// Extensions screen
// Extension manager for installing and configuring extensions
// For bootstrap: renders placeholder content

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_TITLE};

/// Render the extensions management screen
/// For bootstrap: displays placeholder content
/// Future: Implement extension manager:
/// - List of installed extensions
/// - Install/uninstall extensions
/// - Extension configuration
/// - Browse extension marketplace
/// - Extension permissions and sandboxing
pub fn render_extensions_screen(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Extensions");

    let lines = vec![
        Line::from(Span::styled("Extension Manager", STYLE_TITLE)),
        Line::from(""),
        Line::from(Span::styled("Coming soon:", STYLE_MUTED)),
        Line::from(Span::styled("- List of installed extensions", STYLE_MUTED)),
        Line::from(Span::styled("- Install/uninstall extensions", STYLE_MUTED)),
        Line::from(Span::styled("- Extension configuration", STYLE_MUTED)),
        Line::from(Span::styled("- Browse extension marketplace", STYLE_MUTED)),
        Line::from(""),
        Line::from(Span::styled("Add new functionality to Operon with extensions.", STYLE_MUTED)),
    ];

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}
