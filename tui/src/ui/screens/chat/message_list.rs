// Message list widget
// Scrollable chat history with role-based styling and scrollbar
// Displays actual message history from AppState with manual scroll support
// Shows ASCII art banner when no messages are present — scrolls away naturally as chat fills

use crate::state::AppState;
use crate::ui::theme::{
    STYLE_AGENT_MSG, STYLE_INACTIVE_BORDER, STYLE_MUTED, STYLE_TITLE, STYLE_USER_MSG,
};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// ASCII art banner shown at the top of the chat panel on startup
/// Uses the same braille/box characters as the rest of the TUI — no emoji
/// Each string is one line of the banner
const BANNER: &[&str] = &[
    r"    ____                               ",
    r"   / __ \____  ___  _________  ____    ",
    r"  / / / / __ \/ _ \/ ___/ __ \/ __ \   ",
    r" / /_/ / /_/ /  __/ /  / /_/ / / / /   ",
    r" \____/ .___/\___/_/   \____/_/ /_/    ",
    r"     /_/                               ",
];

/// Render the message list (chat history)
/// - When empty: shows the ASCII banner + hint text
/// - When populated: banner is part of the line buffer and scrolls up naturally
///   as new messages push content down and auto-scroll kicks in
pub fn render_message_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_INACTIVE_BORDER)
        .title("Chat");

    // Build the full line buffer that will be passed to Paragraph
    let mut lines: Vec<Line> = Vec::new();

    // Always prepend the banner so it appears at the very top
    // Once enough messages accumulate and auto-scroll is active, it scrolls out of view
    lines.push(Line::from("")); // Top padding before banner
    for banner_line in BANNER {
        lines.push(Line::from(Span::styled(*banner_line, STYLE_TITLE)));
    }
    lines.push(Line::from("")); // Padding after banner

    if state.messages().is_empty() {
        // No messages yet — show hint text below the banner
        lines.push(Line::from(Span::styled(
            "Type a message and press Enter to send (Shift+Enter for newline).",
            STYLE_MUTED,
        )));
        lines.push(Line::from(Span::styled(
            "Type / to switch screens.",
            STYLE_MUTED,
        )));
    } else {
        // Render actual conversation messages
        for message in state.messages() {
            // Map internal role names to display labels
            // "User"  -> "You"
            // "Agent" -> "Operon"
            let (label, style) = if message.role == "User" {
                ("You", STYLE_USER_MSG)
            } else {
                ("Operon", STYLE_AGENT_MSG)
            };

            // Role label on its own styled span, content as plain text
            lines.push(Line::from(vec![
                Span::styled(format!("{}: ", label), style),
                Span::raw(message.content.as_str()),
            ]));
            lines.push(Line::from("")); // Blank line between messages for readability
        }
    }

    // -------------------------------------------------------------------------
    // Scroll calculation
    // We manually count wrapped lines because Paragraph::scroll() works in
    // rendered rows, not logical lines. Without this, long messages cause the
    // scroll position to be off and the last message gets clipped.
    // -------------------------------------------------------------------------
    let text_width = area.width.saturating_sub(2) as usize; // Subtract left+right borders
    let visible_height = area.height.saturating_sub(2) as usize; // Subtract top+bottom borders

    // Count how many terminal rows each logical line will occupy after wrapping
    let mut total_wrapped_lines: usize = 0;
    for line in &lines {
        let line_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        if line_width == 0 {
            // Empty line always takes exactly one row
            total_wrapped_lines += 1;
        } else {
            // Ceiling division: how many rows does this line need?
            let wrapped = (line_width + text_width - 1) / text_width.max(1);
            total_wrapped_lines += wrapped;
        }
    }

    // chat_scroll == 0  →  auto-scroll to bottom (latest message always visible)
    // chat_scroll  > 0  →  user has scrolled up; offset from the bottom
    let max_scroll = total_wrapped_lines.saturating_sub(visible_height) as u16;

    let scroll_offset = if state.chat_scroll() == 0 {
        max_scroll // Pin to bottom
    } else {
        // Subtract from max so that larger chat_scroll = further up
        max_scroll.saturating_sub(state.chat_scroll())
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset, 0));

    frame.render_widget(paragraph, area);

    // Render scrollbar only when content overflows the visible area
    if total_wrapped_lines > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state =
            ScrollbarState::new(total_wrapped_lines.saturating_sub(visible_height))
                .position(scroll_offset as usize);

        frame.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}
