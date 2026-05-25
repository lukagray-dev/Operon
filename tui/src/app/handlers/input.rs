// Input action handlers
// Handles: InputChar, ForwardKeyToInput, InputUndo, InputRedo, SendMessage
// These actions manage the chat input TextArea and message sending

use anyhow::Result;
use crate::events::action::Action;
use crate::state::AppState;
use crate::agent::AgentBridge;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Handle input-related actions
/// Processes character input, key forwarding to TextArea, undo/redo, and message sending
pub async fn handle(
    action: Action,
    state: &mut AppState,
    agent: &Arc<Mutex<Box<dyn AgentBridge>>>,
    tx: &mpsc::Sender<Action>,
) -> Result<()> {
    match action {
        Action::InputChar(c) => {
            // Special case: '/' as first character opens screen selector
            if c == '/' && state.is_input_empty() {
                state.open_screen_selector();
            } else {
                // Forward to TextArea as a regular character
                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                state.message_input_mut().input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            }
        }
        Action::ForwardKeyToInput(key_event) => {
            // Pass the raw key event directly to tui-textarea.
            // Handles arrows, Home, End, Backspace, Delete,
            // Shift+Enter (newline), word-jump (Ctrl+Left/Right), etc.
            // Undo/redo are NOT forwarded — they are handled above
            // via InputUndo/InputRedo which call .undo()/.redo() directly.
            state.message_input_mut().input(key_event);
        }
        Action::InputUndo => {
            // Ctrl+Z — call tui-textarea's undo() directly.
            // tui-textarea's native key for undo is Ctrl+U (Emacs-style),
            // so we bypass key forwarding and call the method directly.
            state.message_input_mut().undo();
        }
        Action::InputRedo => {
            // Ctrl+Shift+Z — call tui-textarea's redo() directly.
            // tui-textarea's native key for redo is Ctrl+R (Emacs-style),
            // so we bypass key forwarding and call the method directly.
            state.message_input_mut().redo();
        }
        Action::SendMessage => {
            // Send message to agent
            let message = state.get_input_text();
            if !message.trim().is_empty() {
                // Add user message to history
                state.add_message("User".to_string(), message.clone());
                state.clear_input();
                
                // Mark agent as thinking — triggers spinner in status bar
                state.set_agent_thinking(true);
                
                // Send to agent asynchronously
                // Clone Arc and tx for the spawned task
                let agent_clone = Arc::clone(agent);
                let action_tx_clone = tx.clone();
                tokio::spawn(async move {
                    let agent_lock = agent_clone.lock().await;
                    match agent_lock.send_message(&message).await {
                        Ok(response) => {
                            let _ = action_tx_clone.send(Action::AgentResponse(response)).await;
                        }
                        Err(e) => {
                            let _ = action_tx_clone.send(Action::AgentResponse(
                                format!("Error: {}", e)
                            )).await;
                        }
                    }
                });
            }
        }
        _ => {
            // Catch-all for safety (should never hit due to dispatch routing)
        }
    }
    
    Ok(())
}
