// Channels screen module
// Channel configuration (WhatsApp, Telegram, Discord, etc.)

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_TITLE};

/// Render the channels configuration screen
/// For bootstrap: displays placeholder content
/// Future: Implement channel manager:
/// - List of available channels (WhatsApp, Telegram, Discord, Gmail, etc.)
/// - Enable/disable toggles
/// - Channel-specific configuration (API keys, webhooks, etc.)
/// - Test connection buttons
/// - Per-channel permission overrides
pub fn render_channels_screen(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Channels");

    let lines = vec![
        Line::from(Span::styled("Channel Configuration", STYLE_TITLE)),
        Line::from(""),
        Line::from(Span::styled("Coming soon:", STYLE_MUTED)),
        Line::from(Span::styled("- WhatsApp integration", STYLE_MUTED)),
        Line::from(Span::styled("- Telegram bot", STYLE_MUTED)),
        Line::from(Span::styled("- Discord bot", STYLE_MUTED)),
        Line::from(Span::styled("- Gmail integration", STYLE_MUTED)),
        Line::from(Span::styled("- Custom webhook channels", STYLE_MUTED)),
        Line::from(""),
        Line::from(Span::styled("Connect Operon to external communication channels.", STYLE_MUTED)),
    ];

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}
