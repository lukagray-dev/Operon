// status_bar.rs — Bottom status bar widget for Operon TUI.
//
// DESIGN & AESTHETICS:
// - Real-time model name, exact context window token capacity, auto-approve mode, and Git branch.
// - Animated braille spinner when agent is processing a turn.
// - High-contrast palette using Operon theme tokens (#60a5fa blue, #f8fafc white, #94a3b8 slate-400).
// - Zero emojis, clean typography with subtle bullet delimiters.

use crate::state::AppState;
use crate::ui::theme::{COLOR_ACCENT, COLOR_LABEL, COLOR_SUCCESS, STYLE_MUTED, STYLE_NORMAL};
use crate::ui::widgets::spinner::get_spinner_frame;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Renders the persistent status bar at the bottom of the screen.
///
/// Format: `[spinner/space] [Model] • ctx: [Used / Capacity (Pct%)] • auto: [off/on] • git:([branch])`
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let session = state.session();
    let mut spans = Vec::new();

    // 1. Thinking Spinner or left spacer
    if state.is_agent_thinking() {
        let frame_char = get_spinner_frame(state.get_tick(), true);
        spans.push(Span::styled(
            format!(" {} ", frame_char),
            Style::default().fg(COLOR_ACCENT),
        ));
    } else {
        spans.push(Span::raw(" "));
    }

    // 2. Active Model Name
    spans.push(Span::styled(
        format!("{} ", session.model_name),
        Style::default()
            .fg(COLOR_ACCENT)
            .add_modifier(Modifier::BOLD),
    ));

    // Separator
    spans.push(Span::styled("• ", STYLE_MUTED));

    // 3. Real Context Window Capacity & Usage
    spans.push(Span::styled(
        format!("ctx: {} ", session.context_display()),
        STYLE_NORMAL,
    ));

    // Separator
    spans.push(Span::styled("• ", STYLE_MUTED));

    // 4. Auto-Approve Indicator (TUI-local toggle)
    let auto_style = if session.auto_approve {
        Style::default()
            .fg(COLOR_SUCCESS)
            .add_modifier(Modifier::BOLD)
    } else {
        STYLE_MUTED
    };
    let auto_text = if session.auto_approve {
        "auto: on "
    } else {
        "auto: off "
    };
    spans.push(Span::styled(auto_text, auto_style));

    // 5. Current Git Branch (if available)
    if session.git_branch != "-" && !session.git_branch.is_empty() {
        spans.push(Span::styled("• ", STYLE_MUTED));
        spans.push(Span::styled(
            format!("git:({})", session.git_branch),
            Style::default().fg(COLOR_LABEL),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
