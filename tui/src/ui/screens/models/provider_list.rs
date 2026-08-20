// provider_list.rs — Provider selection list screen for Operon TUI.
//
// DESIGN & AESTHETICS:
// - Renders all 11 providers dynamically queried from `operon_rs::providers::Provider::all()`.
// - Displays status tags ([Active], [Configured], [API key required], [Not configured]).
// - Shows the currently configured model identifier for the active provider.
// - Clean keyboard navigation with Up/Down and Enter to configure.

use crate::state::AppState;
use crate::ui::theme::{
    COLOR_ACCENT, COLOR_ERROR, COLOR_SUCCESS, COLOR_WARNING, STYLE_ACTIVE_BORDER, STYLE_MUTED,
    STYLE_NORMAL, STYLE_SELECTED,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Renders the full dynamic AI provider selection list.
///
/// Shows all supported providers with their live configuration status and active model.
pub fn render_provider_list(frame: &mut Frame, area: Rect, state: &AppState) {
    // Outer container block with crisp rounded borders
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(STYLE_ACTIVE_BORDER)
        .title(" AI Model Providers ");

    let inner_area = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // Vertical layout: description header, scrollable provider list, footer keybinds
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Top description
            Constraint::Min(5),    // Dynamic provider list
            Constraint::Length(2), // Bottom keybind instructions
        ])
        .split(inner_area);

    // 1. Description Header
    let header_text = vec![
        Line::from(vec![
            Span::styled("Select a provider below to configure API credentials, custom base URLs, and discover models.", STYLE_MUTED),
        ]),
    ];
    let header_widget = Paragraph::new(header_text).alignment(Alignment::Left);
    frame.render_widget(header_widget, chunks[0]);

    // 2. Build List Items from dynamic state
    let selected_index = state.models.selected_provider_index;

    let items: Vec<ListItem> = state
        .models
        .providers
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == selected_index;

            // Indicator pointer
            let pointer = if is_selected { "▶ " } else { "  " };

            // Status tag color styling
            let status_style = if item.is_active {
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if item.status == "Configured" {
                Style::default()
                    .fg(COLOR_SUCCESS)
                    .add_modifier(Modifier::BOLD)
            } else if item.status == "API key required" {
                Style::default().fg(COLOR_WARNING)
            } else {
                Style::default().fg(COLOR_ERROR)
            };

            let row_style = if is_selected {
                STYLE_SELECTED
            } else {
                STYLE_NORMAL
            };

            let mut spans = vec![
                Span::styled(pointer, row_style),
                Span::styled(format!("{:<16}", item.label), row_style),
                Span::raw(" "),
                Span::styled(format!("[{}]", item.status), status_style),
            ];

            // If active model is present, show model ID
            if !item.active_model.is_empty() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("model: {}", item.active_model),
                    STYLE_MUTED,
                ));
            }

            ListItem::new(Line::from(spans)).style(row_style)
        })
        .collect();

    let list_widget = List::new(items).highlight_style(STYLE_SELECTED);

    let mut list_state = ListState::default();
    list_state.select(Some(selected_index));

    frame.render_stateful_widget(list_widget, chunks[1], &mut list_state);

    // 3. Footer Keybind Legend
    let instructions = vec![Line::from(vec![
        Span::styled("[↑/↓]", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Navigate   ", STYLE_MUTED),
        Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Configure Provider   ", STYLE_MUTED),
        Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Back to Chat", STYLE_MUTED),
    ])
    .alignment(Alignment::Center)];

    let instructions_widget = Paragraph::new(instructions);
    frame.render_widget(instructions_widget, chunks[2]);
}
