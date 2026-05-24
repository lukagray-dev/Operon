// Message list widget
// Scrollable chat history with role-based styling and scrollbar
// Displays actual message history from AppState with manual scroll support

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use crate::state::AppState;
use crate::ui::theme::{STYLE_AGENT_MSG, STYLE_INACTIVE_BORDER, STYLE_USER_MSG, STYLE_MUTED};

/// Render the message list (chat history)
/// Displays all messages from AppState with role-based styling
/// Supports manual scrolling with mouse wheel and shows scrollbar
pub fn render_message_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_INACTIVE_BORDER)
        .title("Chat (scroll with mouse wheel)");

    // Build message lines from state
    let mut lines = Vec::new();
    
    if state.messages().is_empty() {
        // Show welcome message when no messages
        lines.push(Line::from(vec![
            Span::styled("Welcome to Operon!", STYLE_MUTED),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Type a message below and press Ctrl+Enter to send.", STYLE_MUTED),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Type / to open the screen selector.", STYLE_MUTED),
        ]));
    } else {
        // Render actual messages
        for message in state.messages() {
            let style = if message.role == "User" {
                STYLE_USER_MSG
            } else {
                STYLE_AGENT_MSG
            };
            
            lines.push(Line::from(vec![
                Span::styled(format!("{}: ", message.role), style),
                Span::raw(&message.content),
            ]));
            lines.push(Line::from("")); // Empty line between messages
        }
    }

    // Calculate actual rendered line count considering text wrapping
    // Available width for text (subtract borders)
    let text_width = area.width.saturating_sub(2) as usize;
    let visible_height = area.height.saturating_sub(2) as usize; // Subtract borders
    
    // Count actual wrapped lines
    let mut total_wrapped_lines = 0;
    for line in &lines {
        // Calculate the display width of this line
        let line_width: usize = line.spans.iter().map(|span| span.content.len()).sum();
        
        if line_width == 0 {
            // Empty line
            total_wrapped_lines += 1;
        } else {
            // Calculate how many lines this will wrap to
            let wrapped_count = (line_width + text_width - 1) / text_width.max(1);
            total_wrapped_lines += wrapped_count;
        }
    }
    
    // Calculate scroll offset
    // If chat_scroll is 0, show the bottom (latest messages)
    // If chat_scroll > 0, scroll up by that amount
    let scroll_offset = if state.chat_scroll() == 0 {
        // Auto-scroll to bottom when scroll is at 0
        if total_wrapped_lines > visible_height {
            (total_wrapped_lines - visible_height) as u16
        } else {
            0
        }
    } else {
        // Manual scroll position
        let max_scroll = if total_wrapped_lines > visible_height {
            (total_wrapped_lines - visible_height) as u16
        } else {
            0
        };
        
        // Scroll from bottom: higher chat_scroll means scrolling up (older messages)
        max_scroll.saturating_sub(state.chat_scroll()).min(max_scroll)
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset, 0));

    frame.render_widget(paragraph, area);

    // Render scrollbar on the right side
    if total_wrapped_lines > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(total_wrapped_lines.saturating_sub(visible_height))
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
