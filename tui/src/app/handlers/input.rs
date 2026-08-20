// Input action handlers
// Handles: InputChar, ForwardKeyToInput, InputUndo, InputRedo, SendMessage
// These actions manage the chat input TextArea and message sending

use crate::agent::AgentBridge;
use crate::events::action::Action;
use crate::state::AppState;
use anyhow::Result;
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
                state
                    .message_input_mut()
                    .input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
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
            state.message_input_mut().redo();
        }
        Action::Paste(text) => {
            handle_paste_text(&text, state);
        }
        Action::PasteClipboard => {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(text) = clipboard.get_text() {
                    handle_paste_text(&text, state);
                }
            }
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

                // Execute prompt through real AgentBridge
                let agent_clone = Arc::clone(agent);
                let action_tx_clone = tx.clone();
                tokio::spawn(async move {
                    let agent_lock = agent_clone.lock().await;
                    if let Err(e) = agent_lock.execute_prompt(message, action_tx_clone.clone()).await {
                        let _ = action_tx_clone
                            .send(Action::AgentError(format!("Prompt execution failed: {}", e)))
                            .await;
                        let _ = action_tx_clone.send(Action::AgentDone).await;
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

/// Routes pasted text to the currently focused input field across all active screens.
pub fn handle_paste_text(text: &str, state: &mut AppState) {
    use crate::state::screen::ActiveScreen;
    use crate::ui::screens::models::state::{ModelsStep, SetupField};

    match state.active_screen() {
        ActiveScreen::Models => {
            if state.models.step == ModelsStep::Setup {
                let single_line = text.trim_matches(|c| c == '\r' || c == '\n').to_string();
                match state.models.focused_field {
                    SetupField::ApiKey => {
                        let clean = single_line.trim().to_string();
                        state.models.api_key_input.insert_str(&clean);
                    }
                    SetupField::BaseUrl => {
                        let clean = single_line.trim().to_string();
                        state.models.base_url_input.insert_str(&clean);
                    }
                    SetupField::CustomModel => {
                        let clean = single_line.trim().to_string();
                        state.models.custom_model_input.insert_str(&clean);
                    }
                    _ => {}
                }
            }
        }
        ActiveScreen::Permissions => {
            if state.permissions.add_dir.open {
                let clean = text.trim_matches(|c| c == '\r' || c == '\n').trim().to_string();
                state.permissions.add_dir.input.insert_str(&clean);
            }
        }
        ActiveScreen::Chat => {
            state.message_input_mut().insert_str(text);
        }
        _ => {}
    }
}
