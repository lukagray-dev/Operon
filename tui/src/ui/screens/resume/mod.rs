// mod.rs — Resume previous conversation screen for Operon TUI.
//
// DESIGN & AESTHETICS:
// - Clean list view showing past conversations discovered for the active workspace.
// - High-contrast selection indicator with model tag and relative timestamp.
// - Zero emojis: uses standard box-drawing and geometric glyphs (`▶`, `•`).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::ui::theme::{
    COLOR_ACCENT, COLOR_LABEL, COLOR_MUTED, STYLE_ACTIVE_BORDER, STYLE_INACTIVE_BORDER,
    STYLE_NORMAL, STYLE_SELECTED,
};

/// Render the Resume Conversation screen.
pub fn render_resume_screen(frame: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header block with workspace info
            Constraint::Min(6),    // Main list of previous conversations
            Constraint::Length(3), // Bottom instructions / keybind bar
        ])
        .split(area);

    // ── Header Block ─────────────────────────────────────────────────────────
    let header_text = vec![
        Span::styled("Workspace: ", Style::default().fg(COLOR_LABEL).add_modifier(Modifier::BOLD)),
        Span::styled(&state.resume.current_workspace, Style::default().fg(COLOR_ACCENT)),
        Span::raw("   "),
        Span::styled(
            format!("({} conversations found)", state.resume.sessions.len()),
            Style::default().fg(COLOR_MUTED),
        ),
    ];
    let header = Paragraph::new(Line::from(header_text)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(STYLE_INACTIVE_BORDER)
            .title(" Resume Previous Conversation "),
    );
    frame.render_widget(header, chunks[0]);

    // ── Session List / Empty State ───────────────────────────────────────────
    if state.resume.sessions.is_empty() {
        let empty_msg = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No previous conversations found for this workspace directory.",
                Style::default().fg(COLOR_MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Start chatting in the Chat screen (press Esc or / -> Chat) to create new sessions.",
                Style::default().fg(COLOR_LABEL),
            )),
        ];
        let empty_block = Paragraph::new(empty_msg).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(STYLE_ACTIVE_BORDER)
                .title(" Previous Sessions "),
        );
        frame.render_widget(empty_block, chunks[1]);
    } else {
        let items: Vec<ListItem> = state
            .resume
            .sessions
            .iter()
            .enumerate()
            .map(|(idx, session)| {
                let is_selected = idx == state.resume.selected_index;
                let (prefix, row_style) = if is_selected {
                    ("▶ ", STYLE_SELECTED)
                } else {
                    ("  ", STYLE_NORMAL)
                };

                let turns_label = if session.turn_count == 1 {
                    "1 turn".to_string()
                } else {
                    format!("{} turns", session.turn_count)
                };

                let line = if is_selected {
                    Line::from(vec![
                        Span::styled(prefix, row_style),
                        Span::styled(format!("\"{}\"  ", session.title), row_style),
                        Span::styled(
                            format!("[{} • {}]  ", session.model_id, turns_label),
                            row_style,
                        ),
                        Span::styled(format!("({})", session.formatted_time), row_style),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(prefix, Style::default().fg(COLOR_MUTED)),
                        Span::styled(
                            format!("\"{}\"  ", session.title),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("[{} • {}]  ", session.model_id, turns_label),
                            Style::default().fg(COLOR_ACCENT),
                        ),
                        Span::styled(
                            format!("({})", session.formatted_time),
                            Style::default().fg(COLOR_MUTED),
                        ),
                    ])
                };

                ListItem::new(line)
            })
            .collect();

        let list_widget = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(STYLE_ACTIVE_BORDER)
                .title(" Select a Conversation to Continue "),
        );
        frame.render_widget(list_widget, chunks[1]);
    }

    // ── Bottom Instructions ──────────────────────────────────────────────────
    let hints = vec![
        Span::styled("[↑/↓] ", Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
        Span::raw("Navigate   "),
        Span::styled("[Enter] ", Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
        Span::raw("Resume Selected   "),
        Span::styled("[Esc] ", Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
        Span::raw("Cancel / Back to Chat   "),
        Span::styled("[/] ", Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
        Span::raw("Switch Screens"),
    ];
    let footer = Paragraph::new(Line::from(hints)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(STYLE_INACTIVE_BORDER),
    );
    frame.render_widget(footer, chunks[2]);
}
