// Help screen
// Keybind reference — scrollable with mouse wheel, same mechanism as chat panel.
// Uses ratatui's Paragraph::scroll() + Scrollbar widget (no manual scroll logic).

use crate::state::AppState;
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_NORMAL, STYLE_TITLE};
use ratatui::{
    layout::{Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Render the help screen with keybind reference.
/// Scrollable via mouse wheel — state.help_scroll() tracks the current offset.
/// The scrollbar is rendered only when content overflows the visible area.
pub fn render_help_screen(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Help");

    // Build the full line buffer — same approach as message_list.rs
    let lines = vec![
        Line::from(Span::styled("Operon TUI - Keybinds", STYLE_TITLE)),
        Line::from(""),
        Line::from(Span::styled("Global:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Ctrl+Q", STYLE_NORMAL),
            Span::styled("          - Quit application", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+T", STYLE_NORMAL),
            Span::styled("          - Toggle terminal panel", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+E", STYLE_NORMAL),
            Span::styled("          - Toggle file explorer", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Esc", STYLE_NORMAL),
            Span::styled("             - Back to Chat screen", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Screen Navigation:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  /", STYLE_NORMAL),
            Span::styled(
                "               - Open screen selector (from Chat)",
                STYLE_MUTED,
            ),
        ]),
        Line::from(vec![
            Span::styled("  Up / Down", STYLE_NORMAL),
            Span::styled("       - Navigate screen selector", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Enter", STYLE_NORMAL),
            Span::styled("           - Confirm screen selection", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Esc", STYLE_NORMAL),
            Span::styled("             - Close screen selector", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Chat - Message Input:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Ctrl+Enter", STYLE_NORMAL),
            Span::styled("      - Send message", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Shift+Enter", STYLE_NORMAL),
            Span::styled("     - Insert newline", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Backspace", STYLE_NORMAL),
            Span::styled("       - Delete character before cursor", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Delete", STYLE_NORMAL),
            Span::styled("          - Delete character at cursor", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Left / Right", STYLE_NORMAL),
            Span::styled("    - Move cursor left / right", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Up / Down", STYLE_NORMAL),
            Span::styled("       - Move cursor between lines", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Home", STYLE_NORMAL),
            Span::styled("            - Move cursor to start of line", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  End", STYLE_NORMAL),
            Span::styled("             - Move cursor to end of line", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+Left/Right", STYLE_NORMAL),
            Span::styled(" - Jump word by word", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+Z", STYLE_NORMAL),
            Span::styled("          - Undo", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+Shift+Z", STYLE_NORMAL),
            Span::styled("    - Redo", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Text Selection (Chat):", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Hold Ctrl+Shift", STYLE_NORMAL),
            Span::styled("  - Enable selection mode", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Drag Mouse", STYLE_NORMAL),
            Span::styled("      - Select text while holding Ctrl+Shift", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C", STYLE_NORMAL),
            Span::styled("          - Copy selected text to clipboard", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Scrolling:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Mouse Wheel", STYLE_NORMAL),
            Span::styled("     - Scroll chat history, input, or help", STYLE_MUTED),
        ]),
        Line::from(""),
        Line::from(Span::styled("Available Screens:", STYLE_TITLE)),
        Line::from(vec![
            Span::styled("  Chat", STYLE_NORMAL),
            Span::styled("            - Main conversation interface", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Models", STYLE_NORMAL),
            Span::styled("          - Configure AI models", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Permissions", STYLE_NORMAL),
            Span::styled("     - Manage agent permissions", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Skills", STYLE_NORMAL),
            Span::styled("          - Agent skills and capabilities", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Extensions", STYLE_NORMAL),
            Span::styled("      - Manage extensions", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Channels", STYLE_NORMAL),
            Span::styled("        - Communication channels", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("  Help", STYLE_NORMAL),
            Span::styled("            - This screen", STYLE_MUTED),
        ]),
    ];

    // -------------------------------------------------------------------------
    // Scroll calculation — identical to message_list.rs
    // Paragraph::scroll() works in rendered rows, so we count wrapped lines.
    // -------------------------------------------------------------------------
    let text_width = area.width.saturating_sub(2) as usize; // subtract borders
    let visible_height = area.height.saturating_sub(2) as usize; // subtract borders

    // Count how many terminal rows each logical line occupies after wrapping
    let mut total_rows: usize = 0;
    for line in &lines {
        let line_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        if line_width == 0 {
            total_rows += 1; // empty line = one row
        } else {
            total_rows += (line_width + text_width - 1) / text_width.max(1);
        }
    }

    // Clamp scroll so we never scroll past the last line
    let max_scroll = total_rows.saturating_sub(visible_height) as u16;
    let scroll_offset = state.help_scroll().min(max_scroll);

    // Keep state in sync with the clamped value (prevents over-scrolling)
    if scroll_offset != state.help_scroll() {
        state.scroll_help_down(0, max_scroll); // clamp via the max guard
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset, 0)); // ratatui built-in scroll — (row_offset, col_offset)

    frame.render_widget(paragraph, area);

    // Render scrollbar only when content overflows the visible area
    if total_rows > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll_offset as usize);

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}
