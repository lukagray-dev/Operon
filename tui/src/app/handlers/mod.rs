// Action dispatch module
// Central dispatcher that routes actions to appropriate handler modules
// Each handler module contains the logic for a specific family of actions

use anyhow::Result;
use std::ops::ControlFlow;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::agent::AgentBridge;
use crate::events::action::Action;
use crate::state::AppState;

// Handler modules - each handles a specific family of actions
mod chat;
mod input;
mod models;
mod mouse;
mod navigation;
mod permissions;
mod resume;

/// Dispatch an action to the appropriate handler
/// Returns ControlFlow::Break to exit the main loop, ControlFlow::Continue to keep running
pub async fn dispatch(
    action: Action,
    state: &mut AppState,
    agent: &Arc<Mutex<Box<dyn AgentBridge>>>,
    tx: &mpsc::Sender<Action>,
    terminal_height: u16,
) -> Result<ControlFlow<(), ()>> {
    match action {
        // Quit action breaks the main loop
        Action::Quit => return Ok(ControlFlow::Break(())),

        // Navigation actions: screen switching, back, screen selector
        Action::Navigate(_)
        | Action::Back
        | Action::CloseScreenSelector
        | Action::ScreenSelectorUp
        | Action::ScreenSelectorDown
        | Action::ScreenSelectorConfirm => navigation::handle(action, state),

        // Input actions: character input, key forwarding, undo/redo, paste, message sending
        Action::InputChar(_)
        | Action::ForwardKeyToInput(_)
        | Action::InputUndo
        | Action::InputRedo
        | Action::Paste(_)
        | Action::PasteClipboard
        | Action::SendMessage => input::handle(action, state, agent, tx).await?,

        // Mouse and selection actions
        Action::ProcessMouse(_)
        | Action::SetCtrlShiftHeld(_)
        | Action::CopySelection => mouse::handle(action, state, terminal_height).await?,

        // Raw key event processing (maps contextually and dispatches directly without channel recursion)
        Action::ProcessKey(key_event) => {
            use crossterm::event::{KeyEventKind, KeyModifiers};
            let ctrl_shift = key_event.modifiers.contains(KeyModifiers::CONTROL)
                && key_event.modifiers.contains(KeyModifiers::SHIFT);

            // Release events update modifier state for mouse selection
            if key_event.kind == KeyEventKind::Release {
                if state.is_ctrl_shift_held() && !ctrl_shift {
                    state.set_ctrl_shift_held(false);
                }
                return Ok(ControlFlow::Continue(()));
            }

            if ctrl_shift != state.is_ctrl_shift_held() {
                state.set_ctrl_shift_held(ctrl_shift);
            }

            let mapped_action = if state.is_screen_selector_open() {
                crate::events::key::map_screen_selector_keys(key_event)
            } else {
                crate::events::key::map_key(key_event, state.active_screen(), state)
            };

            if let Some(inner_action) = mapped_action {
                return Box::pin(dispatch(inner_action, state, agent, tx, terminal_height)).await;
            }
        }

        // Chat actions: agent responses, streaming deltas, cancellation, scrolling, tick
        Action::AgentResponse(_)
        | Action::AgentTextDelta(_)
        | Action::AgentThinkingDelta(_)
        | Action::AgentContextUpdate { .. }
        | Action::AgentDone
        | Action::AgentError(_)
        | Action::CancelPrompt
        | Action::ScrollChatUp(_)
        | Action::ScrollChatDown(_)
        | Action::Tick => chat::handle(action, state, agent).await?,

        // Resume screen actions: navigate past conversations and confirm resumption
        Action::ResumeUp
        | Action::ResumeDown
        | Action::ResumeConfirm => resume::handle(action, state, agent).await?,

        // Models screen actions: provider selection, form input, model fetching, and saving
        Action::ModelsUp
        | Action::ModelsDown
        | Action::ModelsLeft
        | Action::ModelsRight
        | Action::ModelsConfirm
        | Action::ModelsNextField
        | Action::ModelsPrevField
        | Action::ModelsFetchModels
        | Action::ModelsFetchComplete(_)
        | Action::ModelsSaveProvider
        | Action::ModelsSaveComplete(_)
        | Action::ModelsToggleKeyVisibility
        | Action::ModelsForwardKeyToInput(_) => models::handle(action, state, tx).await?,

        // Permissions screen actions: section switching, navigation, modals, permission editing
        Action::PermSwitchSection
        | Action::PermSelectUp
        | Action::PermSelectDown
        | Action::PermToggleExpand
        | Action::PermOpenEditor
        | Action::PermAddDirectory
        | Action::PermDeleteDirectory
        | Action::PermCloseModal
        | Action::PermEditorUp
        | Action::PermEditorDown
        | Action::PermEditorConfirm
        | Action::PermEditorSwitchRole
        | Action::PermForwardKeyToInput(_) => permissions::handle(action, state),

        // Catch-all for any unhandled actions (should be rare)
        _ => {}
    }

    // Continue the main loop
    Ok(ControlFlow::Continue(()))
}
