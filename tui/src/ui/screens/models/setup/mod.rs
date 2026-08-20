// mod.rs — Unified dynamic provider setup & model discovery renderer for Operon TUI.
//
// DESIGN PHILOSOPHY:
// 1. Single Universal Form:
//    - Replaces static per-provider boilerplate with a single capabilities-driven setup screen.
//    - Adapts automatically to API key requirements, base URL overrides, and auth headers.
// 2. Real-Time Model Discovery:
//    - Features an interactive [Fetch Models] trigger connecting to `operon_rs::discover_models`.
//    - Displays live progress spinners and populated model selection lists.
// 3. Zero Emojis & Professional Typography:
//    - Uses clean box glyphs, geometric indicators (`▶`, `•`), and Operon slate/blue theme tokens.

use crate::state::AppState;
use crate::ui::screens::models::state::{FetchStatus, SaveStatus, SetupField};
use crate::ui::theme::{
    COLOR_ACCENT, COLOR_ERROR, COLOR_SUCCESS, COLOR_WARNING, STYLE_ACTIVE_BORDER,
    STYLE_INACTIVE_BORDER, STYLE_MUTED, STYLE_NORMAL, STYLE_SELECTED,
};
use crate::ui::widgets::spinner::get_spinner_frame;
use operon_rs::providers::AuthHeader;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Renders the complete dynamic provider setup and model discovery form.
pub fn render_setup(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let provider = match state.models.selected_provider {
        Some(p) => p,
        None => {
            let error_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Error ");
            frame.render_widget(error_block, area);
            return;
        }
    };

    let capabilities = provider.capabilities();
    let label = provider.display_name();

    // Outer container block
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(STYLE_ACTIVE_BORDER)
        .title(format!(" Configure {} ", label));

    let inner_area = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // Calculate vertical layout chunks
    let has_discovered = !state.models.discovered_models.is_empty();
    let discovered_height = if has_discovered { 7 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),                 // 0: Header / Auth metadata
            Constraint::Length(3),                 // 1: Base URL field
            Constraint::Length(3),                 // 2: API Key field
            Constraint::Length(3),                 // 3: Fetch Models action bar
            Constraint::Length(discovered_height), // 4: Discovered models list (if available)
            Constraint::Length(3),                 // 5: Custom / Selected model input field
            Constraint::Length(3),                 // 6: Save & Activate button + status
            Constraint::Min(1),                    // 7: Spacer
            Constraint::Length(2),                 // 8: Footer keybind instructions
        ])
        .split(inner_area);

    // ─────────────────────────────────────────────────────────────────────────
    // 0. Header & Provider Metadata
    // ─────────────────────────────────────────────────────────────────────────
    let auth_desc = match capabilities.auth_header {
        AuthHeader::Bearer => "Authorization: Bearer <token>",
        AuthHeader::XApiKey => "x-api-key: <key> (Anthropic format)",
        AuthHeader::XGoogApiKey => "x-goog-api-key: <key> (Google format)",
    };

    let meta_spans = vec![
        Span::styled(format!("Provider: {}", label), Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(format!("  •  Auth: {}", auth_desc), STYLE_MUTED),
    ];
    let header_widget = Paragraph::new(Line::from(meta_spans)).alignment(Alignment::Left);
    frame.render_widget(header_widget, chunks[0]);

    // ─────────────────────────────────────────────────────────────────────────
    // 1. Base URL Field
    // ─────────────────────────────────────────────────────────────────────────
    let is_base_focused = state.models.focused_field == SetupField::BaseUrl;
    let base_border_style = if is_base_focused {
        STYLE_ACTIVE_BORDER
    } else {
        STYLE_INACTIVE_BORDER
    };

    let base_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(base_border_style)
        .title(" 1. API Base URL ");

    state.models.base_url_input.set_block(base_block);
    frame.render_widget(&state.models.base_url_input, chunks[1]);

    // ─────────────────────────────────────────────────────────────────────────
    // 2. API Key Field
    // ─────────────────────────────────────────────────────────────────────────
    let is_key_focused = state.models.focused_field == SetupField::ApiKey;
    let key_border_style = if is_key_focused {
        STYLE_ACTIVE_BORDER
    } else {
        STYLE_INACTIVE_BORDER
    };

    let key_title = if state.models.api_key_visible {
        " 2. API Key (Visible - Press F2 to Mask) "
    } else {
        " 2. API Key (Masked - Press F2 to Reveal) "
    };

    let key_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(key_border_style)
        .title(key_title);

    if state.models.api_key_visible {
        state.models.api_key_input.set_block(key_block);
        frame.render_widget(&state.models.api_key_input, chunks[2]);
    } else {
        // Render masked bullets for privacy
        let key_len = state.models.api_key_input.lines().join("").len();
        let masked_str = if key_len == 0 {
            String::new()
        } else {
            "•".repeat(key_len)
        };

        let masked_paragraph = Paragraph::new(masked_str)
            .block(key_block)
            .style(STYLE_NORMAL);
        frame.render_widget(masked_paragraph, chunks[2]);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 3. Model Discovery Trigger & Status Bar
    // ─────────────────────────────────────────────────────────────────────────
    let is_fetch_focused = state.models.focused_field == SetupField::FetchButton;

    let fetch_button_style = if is_fetch_focused {
        STYLE_SELECTED
    } else {
        Style::default()
            .fg(COLOR_ACCENT)
            .add_modifier(Modifier::BOLD)
    };

    let fetch_label = if is_fetch_focused {
        " ▶ [ Fetch Models (Ctrl+F) ] "
    } else {
        "   [ Fetch Models (Ctrl+F) ] "
    };

    let mut fetch_spans = vec![Span::styled(fetch_label, fetch_button_style), Span::raw("  ")];

    // Status indicator
    match &state.models.fetch_status {
        FetchStatus::Idle => {
            fetch_spans.push(Span::styled(
                "Query provider endpoint to auto-discover available models",
                STYLE_MUTED,
            ));
        }
        FetchStatus::Fetching => {
            let spinner = get_spinner_frame(state.get_tick(), true);
            fetch_spans.push(Span::styled(
                format!("{} Discovering models from provider...", spinner),
                Style::default().fg(COLOR_WARNING).add_modifier(Modifier::BOLD),
            ));
        }
        FetchStatus::Success(count) => {
            fetch_spans.push(Span::styled(
                format!("✔ Discovered {} models from provider", count),
                Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD),
            ));
        }
        FetchStatus::Error(err) => {
            fetch_spans.push(Span::styled(
                format!("✖ Discovery error: {}", err),
                Style::default().fg(COLOR_ERROR),
            ));
        }
    }

    let fetch_widget = Paragraph::new(Line::from(fetch_spans)).alignment(Alignment::Left);
    frame.render_widget(fetch_widget, chunks[3]);

    // ─────────────────────────────────────────────────────────────────────────
    // 4. Discovered Models List (if available)
    // ─────────────────────────────────────────────────────────────────────────
    if has_discovered {
        let is_list_focused = state.models.focused_field == SetupField::DiscoveredModelList;
        let list_border = if is_list_focused {
            STYLE_ACTIVE_BORDER
        } else {
            STYLE_INACTIVE_BORDER
        };

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(list_border)
            .title(" Discovered Models (Press Enter to Select) ");

        let list_items: Vec<ListItem> = state
            .models
            .discovered_models
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let is_sel = i == state.models.selected_model_index;
                let pointer = if is_sel { "▶ " } else { "  " };
                let item_style = if is_sel && is_list_focused {
                    STYLE_SELECTED
                } else if is_sel {
                    Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
                } else {
                    STYLE_NORMAL
                };
                let ctx_str = crate::state::session::format_tokens(m.context_window);
                ListItem::new(Line::from(vec![
                    Span::styled(pointer, item_style),
                    Span::styled(format!("{:<36} ", m.model_id), item_style),
                    Span::styled(format!("[ctx: {}]", ctx_str), STYLE_MUTED),
                ]))
            })
            .collect();

        let models_list = List::new(list_items)
            .block(list_block)
            .highlight_style(STYLE_SELECTED);

        let mut list_state = ListState::default();
        list_state.select(Some(state.models.selected_model_index));

        frame.render_stateful_widget(models_list, chunks[4], &mut list_state);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 5. Model Identifier Input Field
    // ─────────────────────────────────────────────────────────────────────────
    let is_model_focused = state.models.focused_field == SetupField::CustomModel;
    let model_border_style = if is_model_focused {
        STYLE_ACTIVE_BORDER
    } else {
        STYLE_INACTIVE_BORDER
    };

    let model_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(model_border_style)
        .title(" 3. Active Model ID (e.g. gpt-4o, claude-3-5-sonnet-latest) ");

    state.models.custom_model_input.set_block(model_block);
    frame.render_widget(&state.models.custom_model_input, chunks[5]);

    // ─────────────────────────────────────────────────────────────────────────
    // 6. Save & Activate Action Button
    // ─────────────────────────────────────────────────────────────────────────
    let is_save_focused = state.models.focused_field == SetupField::SaveButton;
    let save_style = if is_save_focused {
        STYLE_SELECTED
    } else {
        Style::default()
            .fg(COLOR_SUCCESS)
            .add_modifier(Modifier::BOLD)
    };

    let save_label = if is_save_focused {
        " ▶ [ Save & Activate Provider (Ctrl+S / Enter) ] "
    } else {
        "   [ Save & Activate Provider (Ctrl+S / Enter) ] "
    };

    let mut save_spans = vec![Span::styled(save_label, save_style), Span::raw("  ")];

    match &state.models.save_status {
        SaveStatus::Idle => {}
        SaveStatus::Saving => {
            save_spans.push(Span::styled(
                "Persisting to ~/.operon/config.toml...",
                Style::default().fg(COLOR_WARNING),
            ));
        }
        SaveStatus::Success => {
            save_spans.push(Span::styled(
                "✔ Saved and activated successfully!",
                Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD),
            ));
        }
        SaveStatus::Error(err) => {
            save_spans.push(Span::styled(
                format!("✖ Save failed: {}", err),
                Style::default().fg(COLOR_ERROR),
            ));
        }
    }

    let save_widget = Paragraph::new(Line::from(save_spans)).alignment(Alignment::Left);
    frame.render_widget(save_widget, chunks[6]);

    // ─────────────────────────────────────────────────────────────────────────
    // 8. Footer Instructions
    // ─────────────────────────────────────────────────────────────────────────
    let footer_spans = vec![
        Span::styled("[Tab / Shift+Tab]", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Next Field   ", STYLE_MUTED),
        Span::styled("[Ctrl+F]", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Fetch Models   ", STYLE_MUTED),
        Span::styled("[Ctrl+S / Enter]", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Save & Activate   ", STYLE_MUTED),
        Span::styled("[F2]", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Toggle Key Mask   ", STYLE_MUTED),
        Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)),
        Span::styled(" Back to Providers", STYLE_MUTED),
    ];

    let footer_widget = Paragraph::new(Line::from(footer_spans)).alignment(Alignment::Center);
    frame.render_widget(footer_widget, chunks[8]);
}
