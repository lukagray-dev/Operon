// resume.rs — Resume previous conversations action handlers for Operon TUI.
//
// ZERO BUSINESS LOGIC IN FRONTEND:
// When a session is confirmed, loads the persisted turns from operon-rs into AppState
// and registers the session ID with the agent bridge for continuous turns.

use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;

use crate::agent::AgentBridge;
use crate::events::action::Action;
use crate::state::AppState;
use crate::state::screen::ActiveScreen;

/// Handle actions on the Resume Conversation screen.
pub async fn handle(
    action: Action,
    state: &mut AppState,
    agent: &Arc<Mutex<Box<dyn AgentBridge>>>,
) -> Result<()> {
    match action {
        Action::ResumeUp => {
            state.resume.move_up();
        }
        Action::ResumeDown => {
            state.resume.move_down();
        }
        Action::ResumeConfirm => {
            if let Some(session) = state.resume.selected_session() {
                let session_id = session.id.clone();
                if let Err(e) = state.load_session_history(&session_id) {
                    state.add_message(
                        "Operon".to_string(),
                        format!("Failed to load previous conversation: {}", e),
                    );
                } else {
                    let mut agent_lock = agent.lock().await;
                    agent_lock.set_session_id(Some(session_id));
                }
                state.set_active_screen(ActiveScreen::Chat);
            }
        }
        _ => {}
    }

    Ok(())
}
