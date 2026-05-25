// Chat action handlers
// Handles: AgentResponse, ScrollChatUp, ScrollChatDown, Tick
// These actions manage chat message display, scrolling, and animations

use crate::events::action::Action;
use crate::state::AppState;

/// Handle chat-related actions
/// Processes agent responses, chat scrolling, and tick events for animations
pub fn handle(action: Action, state: &mut AppState) {
    match action {
        Action::AgentResponse(response) => {
            // Add agent response to message history and clear thinking state
            state.set_agent_thinking(false);
            state.add_message("Operon".to_string(), response);
        }
        Action::ScrollChatUp(amount) => {
            // Scroll chat up (towards older messages)
            state.scroll_chat_up(amount);
        }
        Action::ScrollChatDown(amount) => {
            // Scroll chat down (towards newer messages)
            state.scroll_chat_down(amount);
        }
        Action::Tick => {
            // Increment tick counter for animations (spinner, etc.)
            state.tick();
        }
        _ => {
            // Catch-all for safety (should never hit due to dispatch routing)
        }
    }
}
