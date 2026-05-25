// Models screen module
// Provider configuration and model selection
// Entry point for the models screen rendering

pub mod state;
pub mod provider_list;
pub mod setup;

use ratatui::{layout::Rect, Frame};
use crate::state::AppState;
use state::ModelsStep;

/// Main render entry point for models screen
/// Called from screens/mod.rs when ActiveScreen::Models is active
/// Routes to provider list or setup form based on current step
pub fn render_models_screen(frame: &mut Frame, area: Rect, state: &mut AppState) {
    match state.models.step {
        ModelsStep::ProviderList => {
            // Screen 1: Show provider selection list
            provider_list::render_provider_list(frame, area, state);
        }
        ModelsStep::Setup => {
            // Screen 2: Show setup form for selected provider
            setup::render_setup(frame, area, state);
        }
    }
}
