// Setup form router (Screen 2)
// Routes to the correct provider-specific setup form based on selected provider

pub mod anthropic;
pub mod openai;
pub mod custom;

use ratatui::{layout::Rect, Frame};
use crate::state::AppState;
use crate::ui::screens::models::state::Provider;

/// Render the setup form for the selected provider
/// Dispatches to provider-specific form renderer
pub fn render_setup(frame: &mut Frame, area: Rect, state: &mut AppState) {
    match state.models.selected_provider {
        Some(Provider::Anthropic) => anthropic::render(frame, area, state),
        Some(Provider::OpenAI) => openai::render(frame, area, state),
        Some(Provider::Custom) => custom::render(frame, area, state),
        None => {
            // Should not happen - if we're on Setup step, a provider must be selected
            // Render empty block as fallback
            use ratatui::widgets::{Block, Borders};
            use crate::ui::theme::STYLE_ACTIVE_BORDER;
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(STYLE_ACTIVE_BORDER)
                .title("Error: No provider selected");
            frame.render_widget(block, area);
        }
    }
}
