// Provider list screen (Screen 1)
// Displays a centered selection list with three provider options
// User navigates with Up/Down and confirms with Enter

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_SELECTED, STYLE_NORMAL, STYLE_MUTED};
use crate::state::AppState;

/// Render the provider selection list
/// Shows three options: Anthropic, OpenAI, Custom
/// Selected item is highlighted with STYLE_SELECTED
pub fn render_provider_list(frame: &mut Frame, area: Rect, state: &AppState) {
    // Create outer block with title and instructions
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Model Providers");

    // Create vertical layout: title space, list, instructions
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Top padding
            Constraint::Min(3),     // Provider list (3 items)
            Constraint::Length(2),  // Bottom instructions
        ])
        .split(block.inner(area));

    // Render outer block
    frame.render_widget(block, area);

    // Build list items for the three providers
    let provider_names = ["Anthropic", "OpenAI", "Custom"];
    let selected_index = state.models.selected_provider_index;

    let items: Vec<ListItem> = provider_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            // Determine style based on selection
            let style = if i == selected_index {
                STYLE_SELECTED
            } else {
                STYLE_NORMAL
            };

            // Add selection indicator (▶) for selected item
            let prefix = if i == selected_index { "▶ " } else { "  " };
            
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(*name, style),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    // Create List widget with ListState for proper selection rendering
    let list = List::new(items)
        .highlight_style(STYLE_SELECTED);

    // Create ListState to track selection
    let mut list_state = ListState::default();
    list_state.select(Some(selected_index));

    // Render the list
    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    // Render instructions at the bottom
    let instructions = vec![
        Line::from(vec![
            Span::styled("[Enter]", STYLE_NORMAL),
            Span::styled(" Select   ", STYLE_MUTED),
            Span::styled("[Esc]", STYLE_NORMAL),
            Span::styled(" Back", STYLE_MUTED),
        ])
        .alignment(Alignment::Center),
    ];

    let instructions_widget = ratatui::widgets::Paragraph::new(instructions);
    frame.render_widget(instructions_widget, chunks[2]);
}
