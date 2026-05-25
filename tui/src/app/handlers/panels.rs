// Panel action handlers
// Handles: ToggleTerminal, ToggleLeftSidebar, ToggleRightPanel, CloseRightPanel, OpenFile
// These actions manage the visibility and content of UI panels (left sidebar, right panel)

use crate::events::action::Action;
use crate::state::AppState;

/// Handle panel-related actions
/// Processes panel visibility toggles and content changes
pub fn handle(action: Action, state: &mut AppState) {
    match action {
        Action::ToggleTerminal => {
            // Toggle terminal panel (open if closed, close if open)
            use crate::ui::chrome::right_sidebar::panel_state::RightPanelContent;
            if let Some(RightPanelContent::Terminal) = state.right_panel() {
                state.set_right_panel(None);
            } else {
                state.set_right_panel(Some(RightPanelContent::Terminal));
            }
        }
        Action::ToggleLeftSidebar => {
            // Toggle left sidebar (file explorer) (open if closed, close if open)
            state.toggle_left_sidebar();
        }
        Action::ToggleRightPanel(content) => {
            // Open right panel with specified content
            state.set_right_panel(Some(content));
        }
        Action::CloseRightPanel => {
            // Hide right panel
            state.set_right_panel(None);
        }
        Action::OpenFile(path) => {
            // Open file preview in right panel
            use crate::ui::chrome::right_sidebar::panel_state::RightPanelContent;
            state.set_right_panel(Some(RightPanelContent::FilePreview(path)));
        }
        _ => {
            // Catch-all for safety (should never hit due to dispatch routing)
        }
    }
}
