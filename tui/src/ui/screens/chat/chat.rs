// Chat screen
// Main chat interface with message history and input box
// Composes message_list and input_box into a vertical layout

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use crate::state::AppState;
use super::{input_box, message_list, screen_selector};

/// Render the chat screen
/// Layout when screen selector is closed:
/// ```
/// ┌─────────────────────────┐
/// │                         │
/// │    Message History      │
/// │    (scrollable)         │
/// │                         │
/// ├─────────────────────────┤
/// │    Input Box (3 lines)  │
/// └─────────────────────────┘
/// ```
/// 
/// Layout when screen selector is open:
/// ```
/// ┌─────────────────────────┐
/// │                         │
/// │    Message History      │
/// │    (scrollable)         │
/// │                         │
/// ├─────────────────────────┤
/// │  Screen Selector (9 ln) │
/// ├─────────────────────────┤
/// │    Input Box (3 lines)  │
/// └─────────────────────────┘
/// ```
pub fn render_chat_screen(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if state.is_screen_selector_open() {
        // Layout with screen selector: message history | selector | input
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),      // Message history (takes remaining space)
                Constraint::Length(11),  // Screen selector (9 lines: 7 screens + 2 borders + title)
                Constraint::Length(5),   // Input box (3 visible + 2 borders)
            ])
            .split(area);

        // Render message history
        message_list::render_message_list(frame, chunks[0], state);

        // Render screen selector
        screen_selector::render_screen_selector(frame, chunks[1], state.screen_selector_index());

        // Render input box with TextArea widget
        input_box::render_input_box(frame, chunks[2], state);
    } else {
        // Normal layout: message history | input
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),      // Message history (takes remaining space)
                Constraint::Length(5),   // Input box (3 visible + 2 borders)
            ])
            .split(area);

        // Render message history
        message_list::render_message_list(frame, chunks[0], state);

        // Render input box with TextArea widget
        input_box::render_input_box(frame, chunks[1], state);
    }
}
