// Status bar widget
// Bottom bar showing model name, context usage, active screen, agent status
// Always visible at the bottom of the terminal

use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::state::AppState;
use crate::ui::theme::STYLE_STATUSBAR;

/// Render the status bar at the bottom of the screen
/// Layout: [Model] • [ctx: X/Y (Z%)]
/// Example: "claude-sonnet-4.5 • ctx: 45.2K/200K (22%)"
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let session = state.session();
    
    // Build status bar content
    let mut spans = Vec::new();

    // Model name
    spans.push(Span::styled(
        format!(" {} ", session.model_name),
        STYLE_STATUSBAR,
    ));

    // Separator
    spans.push(Span::styled(" • ", STYLE_STATUSBAR));

    // Context usage (abbreviated as "ctx")
    spans.push(Span::styled(
        format!("ctx: {} ", session.context_display()),
        STYLE_STATUSBAR,
    ));

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
