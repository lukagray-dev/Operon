// Screens module
// All full-page screens that can be displayed in the main panel
// Each screen is swapped in based on AppState.active_screen

pub mod channels;
pub mod chat;
pub mod extensions;
pub mod help;
pub mod models;
pub mod permissions;
pub mod skills;

use ratatui::{layout::Rect, Frame};
use crate::state::{screen::ActiveScreen, AppState};

/// Render the currently active screen
/// Dispatches to the appropriate screen renderer based on AppState.active_screen
pub fn render_active_screen(frame: &mut Frame, area: Rect, state: &mut AppState) {
    match state.active_screen() {
        ActiveScreen::Chat => {
            // Chat screen handles screen selector rendering internally
            chat::render_chat_screen(frame, area, state);
        }
        ActiveScreen::Models => models::render_models_screen(frame, area, state),
        ActiveScreen::Permissions => permissions::render_permissions_screen(frame, area, state),
        ActiveScreen::Skills => skills::render_skills_screen(frame, area),
        ActiveScreen::Extensions => extensions::render_extensions_screen(frame, area),
        ActiveScreen::Channels => channels::render_channels_screen(frame, area),
        ActiveScreen::Help => help::render_help_screen(frame, area, state),
    }
}
