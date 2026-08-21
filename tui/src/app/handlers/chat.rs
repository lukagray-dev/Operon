// Chat action handlers
// Handles: AgentResponse, AgentTextDelta, AgentThinkingDelta, AgentContextUpdate, AgentDone, AgentError, CancelPrompt, ScrollChatUp, ScrollChatDown, Tick
// These actions manage real-time chat streaming, token tracking, prompt cancellation, and scrolling.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::agent::AgentBridge;
use crate::events::action::Action;
use crate::state::AppState;

/// Handle chat-related actions
/// Processes streaming agent deltas, token usage gauges, cancellation requests, and animation ticks.
pub async fn handle(
    action: Action,
    state: &mut AppState,
    agent: &Arc<Mutex<Box<dyn AgentBridge>>>,
) -> Result<()> {
    match action {
        Action::AgentResponse(response) => {
            state.set_agent_thinking(false);
            state.add_message("Operon".to_string(), response);
        }
        Action::AgentTextDelta(delta) => {
            state.append_assistant_delta(&delta);
        }
        Action::AgentThinkingDelta(delta) => {
            state.append_thinking_delta(&delta);
        }
        Action::AgentContextUpdate {
            current_tokens,
            total_window,
        } => {
            state.update_context_usage(current_tokens, total_window);
        }
        Action::AgentDone => {
            state.set_agent_thinking(false);
        }
        Action::AgentError(err) => {
            state.append_assistant_delta(&format!("\n[Error: {}]\n", err));
            state.set_agent_thinking(false);
        }
        Action::CancelPrompt => {
            state.cancel_in_flight_generation();
            let agent_lock = agent.lock().await;
            let _ = agent_lock.cancel().await;
        }
        Action::ScrollChatUp(amount) => {
            state.scroll_chat_up(amount);
        }
        Action::ScrollChatDown(amount) => {
            state.scroll_chat_down(amount);
        }
        Action::Tick => {
            state.tick();
        }
        _ => {}
    }
    Ok(())
}
