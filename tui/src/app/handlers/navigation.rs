// Navigation action handlers
// Handles: Navigate, Back, CloseScreenSelector, ScreenSelector{Up,Down,Confirm}
// These actions control screen switching and the screen selector popup

use crate::events::action::Action;
use crate::state::AppState;

/// Handle navigation-related actions
/// Processes screen switching, back navigation, and screen selector interactions
pub fn handle(action: Action, state: &mut AppState) {
    match action {
        Action::Navigate(screen) => {
            // Switch to a different screen
            state.set_active_screen(screen);
        }
        Action::Back => {
            // If on Models Setup screen, go back to ProviderList
            if state.active_screen() == &crate::state::screen::ActiveScreen::Models
                && state.models.step == crate::ui::screens::models::state::ModelsStep::Setup
            {
                state.models.back_to_provider_list();
            } else {
                // Otherwise return to Chat screen
                state.set_active_screen(crate::state::screen::ActiveScreen::Chat);
            }
        }
        Action::CloseScreenSelector => {
            // Close screen selector popup
            state.close_screen_selector();
        }
        Action::ScreenSelectorUp => {
            // Move selection up in screen selector
            state.screen_selector_up();
        }
        Action::ScreenSelectorDown => {
            // Move selection down in screen selector
            state.screen_selector_down();
        }
        Action::ScreenSelectorConfirm => {
            // Confirm screen selector selection and navigate to selected screen
            let selected = state.get_selected_screen();
            state.set_active_screen(selected);
            state.close_screen_selector();
        }
        _ => {
            // Catch-all for safety (should never hit due to dispatch routing)
        }
    }
}
