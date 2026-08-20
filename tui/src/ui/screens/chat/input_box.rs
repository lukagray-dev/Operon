// Input box widget
// Multi-line text editor for composing messages using tui-textarea
// TextArea handles all cursor movement, editing, undo/redo automatically

use crate::state::AppState;
use crate::ui::theme::STYLE_ACTIVE_BORDER;
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders},
    Frame,
};

/// Render the input box for composing messages
/// Uses tui-textarea's TextArea widget which provides production-grade text editing:
/// - Full cursor movement (arrows, home, end, etc.)
/// - Character insertion/deletion at cursor position
/// - Multi-line editing with proper line navigation
/// - Undo/redo support (Ctrl+Z, Ctrl+Y)
/// - Selection and clipboard operations
///
/// Keybinds:
/// - Type to input text at cursor
/// - Shift+Enter: New line
/// - Ctrl+Enter: Send message
/// - Backspace/Delete: Delete characters
/// - Arrow keys: Move cursor
/// - Home/End: Jump to start/end of line
/// - Ctrl+A: Select all
/// - Ctrl+Z/Y: Undo/redo
/// - /: Open screen selector (when input is empty)
pub fn render_input_box(frame: &mut Frame, area: Rect, state: &mut AppState) {
    // Check state before borrowing textarea mutably
    let is_empty = state.is_input_empty();
    let is_thinking = state.agent_thinking();

    // Get mutable reference to TextArea for configuration
    {
        let textarea = state.message_input_mut();

        // Set dynamic title based on thinking state
        let title = if is_thinking {
            "Agent Thinking / Generating... (Esc: cancel)"
        } else {
            "Message (Enter: send, Shift+Enter: newline, /: screens)"
        };

        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(STYLE_ACTIVE_BORDER)
                .title(title),
        );

        // Set placeholder text when empty
        if is_empty {
            textarea.set_placeholder_text("Type your message here...");
        }
    }

    // Render the TextArea widget with immutable reference
    // TextArea handles cursor rendering, scrolling, and all text editing internally
    frame.render_widget(state.message_input(), area);
}
