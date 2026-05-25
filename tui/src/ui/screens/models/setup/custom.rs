// Custom provider setup form
// Editable base URL, compatibility mode selector, API key input, model fetch

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use crate::state::AppState;
use crate::ui::screens::models::state::{CompatibilityMode, FetchStatus};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_NORMAL, STYLE_SELECTED};
use crate::ui::widgets::spinner;

/// Render Custom provider setup form
/// Layout: Base URL (editable) | Compatibility mode | API Key (editable) | Models section
pub fn render(frame: &mut Frame, area: Rect, state: &mut AppState) {
    // Create outer block with title
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Custom Provider");

    // Split into sections: base URL, compat mode, API key, models, instructions
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Base URL section (label + value)
            Constraint::Length(1),  // Spacing
            Constraint::Length(3),  // Compatibility mode section
            Constraint::Length(1),  // Spacing
            Constraint::Length(3),  // API Key section (label + value)
            Constraint::Length(1),  // Spacing
            Constraint::Length(1),  // Models separator
            Constraint::Min(5),     // Models section (fetch button or list)
            Constraint::Length(3),  // Instructions
        ])
        .split(block.inner(area));

    // Render outer block
    frame.render_widget(block, area);

    // --- Base URL section (editable) ---
    let is_url_focused = state.models.focused_field == 0;
    
    // Configure the TextArea widget for base URL
    {
        let textarea = &mut state.models.base_url_input;
        let border_style = if is_url_focused {
            STYLE_ACTIVE_BORDER
        } else {
            STYLE_MUTED
        };
        
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Base URL")
        );
        
        // Set placeholder when empty
        if textarea.is_empty() {
            textarea.set_placeholder_text("http://localhost:11434");
        }
    }
    
    // Render the TextArea widget
    frame.render_widget(&state.models.base_url_input, chunks[0]);

    // --- Compatibility mode section ---
    let is_compat_focused = state.models.focused_field == 1;
    let compat_border_style = if is_compat_focused {
        STYLE_ACTIVE_BORDER
    } else {
        STYLE_MUTED
    };

    // Build compatibility mode display with radio buttons
    let (openai_marker, anthropic_marker) = match state.models.compat_mode {
        CompatibilityMode::OpenAICompatible => ("●", " "),
        CompatibilityMode::AnthropicCompatible => (" ", "●"),
    };

    let compat_lines = vec![
        Line::from(Span::styled("Compatibility", STYLE_NORMAL)),
        Line::from(vec![
            Span::styled("[", STYLE_MUTED),
            Span::styled(openai_marker, STYLE_NORMAL),
            Span::styled("] OpenAI-compatible  [", STYLE_MUTED),
            Span::styled(anthropic_marker, STYLE_NORMAL),
            Span::styled("] Anthropic", STYLE_MUTED),
        ]),
    ];

    let compat_block = Block::default()
        .borders(Borders::ALL)
        .border_style(compat_border_style);
    let compat_widget = Paragraph::new(compat_lines).block(compat_block);
    frame.render_widget(compat_widget, chunks[2]);

    // --- API Key section ---
    let is_key_focused = state.models.focused_field == 2;
    
    // Configure the TextArea widget for API key
    {
        let textarea = &mut state.models.api_key_input;
        let border_style = if is_key_focused {
            STYLE_ACTIVE_BORDER
        } else {
            STYLE_MUTED
        };
        
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("API Key")
        );
        
        // Set placeholder when empty
        if textarea.is_empty() {
            textarea.set_placeholder_text("Enter your API key...");
        }
        
        // Mask the input if not visible
        if !state.models.api_key_visible {
            textarea.set_mask_char('•');
        } else {
            textarea.set_mask_char('\0'); // No masking
        }
    }
    
    // Render the TextArea widget
    frame.render_widget(&state.models.api_key_input, chunks[4]);

    // --- Models separator ---
    let separator = Line::from(Span::styled("── Models ────────────────────────────", STYLE_MUTED));
    frame.render_widget(Paragraph::new(separator), chunks[6]);

    // --- Models section ---
    render_models_section(frame, chunks[7], state);

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
    frame.render_widget(instructions_widget, chunks[8]);
}

/// Render the models section (fetch button or model list)
fn render_models_section(frame: &mut Frame, area: Rect, state: &AppState) {
    match &state.models.fetch_status {
        FetchStatus::Idle => {
            // Show instruction to press Enter on API key field
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled("Enter API key and press Enter to fetch models", STYLE_MUTED)),
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
                Line::from(Span::styled(format!("Error: {}", err), crate::ui::theme::STYLE_ERROR)),
                Line::from(""),
                Line::from(Span::styled("Press Enter on API key field to retry", STYLE_MUTED)),
            ];
            let widget = Paragraph::new(lines);
            frame.render_widget(widget, area);
        }
    }
}
