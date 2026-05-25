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
mod navigation;
mod input;
mod chat;
mod mouse;
mod panels;
mod models;
mod permissions;

/// Dispatch an action to the appropriate handler
/// Returns ControlFlow::Break to exit the main loop, ControlFlow::Continue to keep running
/// 
/// This function contains ONLY the top-level match statement - no logic.
/// Every arm calls one handler module that contains the actual implementation.
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
        Action::Navigate(_) | Action::Back | Action::CloseScreenSelector
        | Action::ScreenSelectorUp | Action::ScreenSelectorDown
        | Action::ScreenSelectorConfirm
            => navigation::handle(action, state),

        // Input actions: character input, key forwarding, undo/redo, message sending
        Action::InputChar(_) | Action::ForwardKeyToInput(_)
        | Action::InputUndo | Action::InputRedo | Action::SendMessage
            => input::handle(action, state, agent, tx).await?,

        // Mouse and keyboard actions: mouse events, selection mode, raw key processing
        Action::ProcessMouse(_) | Action::SetCtrlShiftHeld(_) | Action::CopySelection
        | Action::ProcessKey(_)
            => mouse::handle(action, state, tx, terminal_height).await?,

        // Panel actions: toggle terminal, sidebar, right panel
        Action::ToggleTerminal | Action::ToggleLeftSidebar | Action::ToggleRightPanel(_)
        | Action::CloseRightPanel | Action::OpenFile(_)
            => panels::handle(action, state),

        // Chat actions: agent responses, scrolling, tick
        Action::AgentResponse(_) | Action::ScrollChatUp(_) | Action::ScrollChatDown(_)
        | Action::Tick
            => chat::handle(action, state),

        // Models screen actions: provider selection, form input, model fetching
        Action::ModelsUp | Action::ModelsDown | Action::ModelsLeft | Action::ModelsRight
        | Action::ModelsConfirm | Action::ModelsNextField | Action::ModelsFetchModels
        | Action::ModelsFetchComplete(_) | Action::ModelsToggleCompat
        | Action::ModelsForwardKeyToInput(_)
            => models::handle(action, state, tx).await?,

        // Permissions screen actions: section switching, navigation, modals, permission editing
        Action::PermSwitchSection | Action::PermSelectUp | Action::PermSelectDown
        | Action::PermToggleExpand | Action::PermOpenEditor | Action::PermAddDirectory
        | Action::PermDeleteDirectory | Action::PermCloseModal | Action::PermEditorUp
        | Action::PermEditorDown | Action::PermEditorConfirm | Action::PermEditorSwitchRole
        | Action::PermForwardKeyToInput(_)
            => permissions::handle(action, state),

        // Catch-all for any unhandled actions (should be rare)
        _ => {}
    }

    // Continue the main loop
    Ok(ControlFlow::Continue(()))
}
