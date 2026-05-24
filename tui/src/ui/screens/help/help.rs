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
            Span::styled("  Ctrl+T", STYLE_NORMAL),
            Span::styled(" - Toggle terminal panel", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Esc", STYLE_NORMAL),
            Span::styled(" - Back to Chat screen", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Screen Navigation:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  /", STYLE_NORMAL),
            Span::styled(" - Open screen selector (from Chat)", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  ↑/↓", STYLE_NORMAL),
            Span::styled(" - Navigate screen selector", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Enter", STYLE_NORMAL),
            Span::styled(" - Confirm screen selection", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Esc", STYLE_NORMAL),
            Span::styled(" - Close screen selector", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Chat - Message Input:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Ctrl+Enter", STYLE_NORMAL),
            Span::styled(" - Send message", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Shift+Enter", STYLE_NORMAL),
            Span::styled(" - Insert newline", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Backspace", STYLE_NORMAL),
            Span::styled(" - Delete character before cursor", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Delete", STYLE_NORMAL),
            Span::styled(" - Delete character at cursor", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  ←/→", STYLE_NORMAL),
            Span::styled(" - Move cursor left/right", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  ↑/↓", STYLE_NORMAL),
            Span::styled(" - Move cursor up/down lines", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Home", STYLE_NORMAL),
            Span::styled(" - Move cursor to start of line", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  End", STYLE_NORMAL),
            Span::styled(" - Move cursor to end of line", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+A", STYLE_NORMAL),
            Span::styled(" - Select all", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+Z", STYLE_NORMAL),
            Span::styled(" - Undo", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+Y", STYLE_NORMAL),
            Span::styled(" - Redo", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Text Selection:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Hold Ctrl+Shift", STYLE_NORMAL),
            Span::styled(" - Enable selection mode", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Drag Mouse", STYLE_NORMAL),
            Span::styled(" - Select text while holding Ctrl+Shift", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Release Keys", STYLE_NORMAL),
            Span::styled(" - Auto-copy to clipboard", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Chat - Scrolling:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Mouse Wheel", STYLE_NORMAL),
            Span::styled(" - Scroll chat history or input", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Available Screens:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Chat", STYLE_NORMAL),
            Span::styled(" - Main conversation interface", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Models", STYLE_NORMAL),
            Span::styled(" - Configure AI models", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Permissions", STYLE_NORMAL),
            Span::styled(" - Manage agent permissions", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Skills", STYLE_NORMAL),
            Span::styled(" - Agent skills and capabilities", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Extensions", STYLE_NORMAL),
            Span::styled(" - Manage extensions", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Channels", STYLE_NORMAL),
            Span::styled(" - Communication channels", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Help", STYLE_NORMAL),
            Span::styled(" - This screen", STYLE_MUTED),
        ]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
