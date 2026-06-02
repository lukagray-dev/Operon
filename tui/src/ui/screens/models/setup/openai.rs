// OpenAI setup form
// Fixed base URL (https://api.openai.com), API key input, model fetch
// Identical structure to Anthropic form, only differs in title and base URL

use crate::state::AppState;
use crate::ui::screens::models::state::FetchStatus;
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_NORMAL, STYLE_SELECTED};
use crate::ui::widgets::spinner;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Render OpenAI setup form
/// Layout: Base URL (read-only) | API Key (editable) | Models section
pub fn render(frame: &mut Frame, area: Rect, state: &mut AppState) {
    // Create outer block with title
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("OpenAI");

    // Split into sections: base URL, API key, models, instructions
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Base URL section (label + value)
            Constraint::Length(1), // Spacing
            Constraint::Length(3), // API Key section (label + value)
            Constraint::Length(1), // Spacing
            Constraint::Length(1), // Models separator
            Constraint::Min(5),    // Models section (fetch button or list)
            Constraint::Length(3), // Instructions
        ])
        .split(block.inner(area));

    // Render outer block
    frame.render_widget(block, area);

    // --- Base URL section (read-only) ---
    let base_url_lines = vec![
        Line::from(Span::styled("Base URL", STYLE_NORMAL)),
        Line::from(Span::styled("https://api.openai.com", STYLE_MUTED)),
    ];
    let base_url_widget = Paragraph::new(base_url_lines);
    frame.render_widget(base_url_widget, chunks[0]);

    // --- API Key section ---
    // Render the TextArea widget for API key input
    let is_focused = state.models.focused_field == 0;

    // Configure the TextArea widget
    {
        let textarea = &mut state.models.api_key_input;
        let border_style = if is_focused {
            STYLE_ACTIVE_BORDER
        } else {
            STYLE_MUTED
        };

        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("API Key"),
        );

        // Set placeholder when empty
        if textarea.is_empty() {
            textarea.set_placeholder_text("Enter your OpenAI API key...");
        }

        // Mask the input if not visible
        if !state.models.api_key_visible {
            textarea.set_mask_char('•');
        } else {
            textarea.set_mask_char('\0'); // No masking
        }
    }

    // Render the TextArea widget
    frame.render_widget(&state.models.api_key_input, chunks[2]);

    // --- Models separator ---
    let separator = Line::from(Span::styled(
        "── Models ────────────────────────────",
        STYLE_MUTED,
    ));
    frame.render_widget(Paragraph::new(separator), chunks[4]);

    // --- Models section ---
    render_models_section(frame, chunks[5], state);

    // --- Instructions ---
    let instructions = vec![
        Line::from(vec![
            Span::styled("[Tab]", STYLE_NORMAL),
            Span::styled(" Next field  ", STYLE_MUTED),
            Span::styled("[Enter]", STYLE_NORMAL),
            Span::styled(" Fetch models", STYLE_MUTED),
        ]),
        Line::from(vec![
            Span::styled("[Esc]", STYLE_NORMAL),
            Span::styled(" Back", STYLE_MUTED),
        ]),
    ];
    let instructions_widget = Paragraph::new(instructions);
    frame.render_widget(instructions_widget, chunks[6]);
}

/// Render the models section (fetch button or model list)
fn render_models_section(frame: &mut Frame, area: Rect, state: &AppState) {
    match &state.models.fetch_status {
        FetchStatus::Idle => {
            // Show instruction to press Enter on API key field
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Enter API key and press Enter to fetch models",
                    STYLE_MUTED,
                )),
            ];
            let widget = Paragraph::new(lines);
            frame.render_widget(widget, area);
        }
        FetchStatus::Fetching => {
            // Show spinner
            let spinner_char = spinner::get_spinner_frame(state.get_tick(), true);
            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(format!("{} ", spinner_char), STYLE_NORMAL),
                    Span::styled("Fetching models...", STYLE_MUTED),
                ]),
            ];
            let widget = Paragraph::new(lines);
            frame.render_widget(widget, area);
        }
        FetchStatus::Success => {
            // Show model list with selection
            if state.models.fetched_models.is_empty() {
                let lines = vec![Line::from(Span::styled("No models available", STYLE_MUTED))];
                let widget = Paragraph::new(lines);
                frame.render_widget(widget, area);
            } else {
                let items: Vec<ListItem> = state
                    .models
                    .fetched_models
                    .iter()
                    .enumerate()
                    .map(|(i, model)| {
                        let style = if i == state.models.selected_model_index {
                            STYLE_SELECTED
                        } else {
                            STYLE_NORMAL
                        };
                        let prefix = if i == state.models.selected_model_index {
                            "▶ "
                        } else {
                            "  "
                        };
                        let line = Line::from(vec![
                            Span::styled(prefix, style),
                            Span::styled(model.clone(), style),
                        ]);
                        ListItem::new(line).style(style)
                    })
                    .collect();

                let list = List::new(items).highlight_style(STYLE_SELECTED);
                let mut list_state = ListState::default();
                list_state.select(Some(state.models.selected_model_index));
                frame.render_stateful_widget(list, area, &mut list_state);
            }
        }
        FetchStatus::Error(err) => {
            // Show error message
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("Error: {}", err),
                    crate::ui::theme::STYLE_ERROR,
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter on API key field to retry",
                    STYLE_MUTED,
                )),
            ];
            let widget = Paragraph::new(lines);
            frame.render_widget(widget, area);
        }
    }
}
