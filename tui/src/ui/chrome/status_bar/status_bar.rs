// Status bar widget
// Bottom bar showing model name, context usage, active screen, agent status
// Always visible at the bottom of the terminal

use crate::state::AppState;
use crate::ui::theme::STYLE_INACTIVE_BORDER;
use crate::ui::widgets::spinner::get_spinner_frame;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Render the status bar at the bottom of the screen
/// Layout when idle:    [Model] • [ctx: X/Y (Z%)] • [Mouse: ON/OFF]
/// Layout when thinking: [spinner] [Model] • [ctx: X/Y (Z%)] • [Mouse: ON/OFF]
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let session = state.session();
    let mut spans = Vec::new();

    // Show animated spinner before model name while agent is generating a response
    if state.is_agent_thinking() {
        // get_spinner_frame advances through braille frames based on tick count
        let frame_char = get_spinner_frame(state.get_tick(), true);
        spans.push(Span::styled(
            format!(" {} ", frame_char),
            STYLE_INACTIVE_BORDER,
        ));
    } else {
        // Empty spacer so model name stays at a consistent position
        spans.push(Span::styled(" ", STYLE_INACTIVE_BORDER));
    }

    // Model name
    spans.push(Span::styled(
        format!("{} ", session.model_name),
        STYLE_INACTIVE_BORDER,
    ));

    // Separator
    spans.push(Span::styled("• ", STYLE_INACTIVE_BORDER));

    // Context usage (abbreviated as "ctx")
    spans.push(Span::styled(
        format!("ctx: {}", session.context_display()),
        STYLE_INACTIVE_BORDER,
    ));

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
