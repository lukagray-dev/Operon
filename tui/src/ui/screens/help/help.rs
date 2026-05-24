// Help screen
// Keybind reference and searchable help content
// For bootstrap: renders basic keybind list

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_TITLE, STYLE_NORMAL, STYLE_MUTED};

/// Render the help screen with keybind reference
/// For bootstrap: displays static keybind list
/// Future: Implement searchable help:
/// - Search bar for filtering keybinds
/// - Context-sensitive help (different keybinds per screen)
/// - Help topics and tutorials
/// - Link to online documentation
pub fn render_help_screen(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Help");

    let lines = vec![
        Line::from(Span::styled("Operon TUI - Keybinds", STYLE_TITLE)),
        Line::from(""),
        Line::from(Span::styled("Global Keybinds:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Ctrl+Q", STYLE_NORMAL),
            Span::styled(" - Quit application", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C", STYLE_NORMAL),
            Span::styled(" - Quit application", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Tab", STYLE_NORMAL),
            Span::styled(" - Next screen", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Shift+Tab", STYLE_NORMAL),
            Span::styled(" - Previous screen", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  F1-F7", STYLE_NORMAL),
            Span::styled(" - Jump to specific screen", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+T", STYLE_NORMAL),
            Span::styled(" - Toggle terminal panel", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Esc", STYLE_NORMAL),
            Span::styled(" - Close right panel", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Chat Screen:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Ctrl+Enter", STYLE_NORMAL),
            Span::styled(" - Send message", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Screen Navigation:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  F1", STYLE_NORMAL),
            Span::styled(" - Chat", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  F2", STYLE_NORMAL),
            Span::styled(" - Models", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  F3", STYLE_NORMAL),
            Span::styled(" - Permissions", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  F4", STYLE_NORMAL),
            Span::styled(" - Skills", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  F5", STYLE_NORMAL),
            Span::styled(" - Extensions", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  F6", STYLE_NORMAL),
            Span::styled(" - Channels", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  F7", STYLE_NORMAL),
            Span::styled(" - Help", STYLE_MUTED),
        ]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
