//! Permission callback wiring and session command triggers.
//!
//! Maps user approval and denial actions from interactive policy checks
//! in the GUI back to the running agent session.

use crate::state::AppState;
use std::rc::Rc;
use std::cell::RefCell;

/// Registers permission approval and denial callbacks on the Slint window.
///
/// This binds the callbacks exposed by the window's main content area.
pub fn wire_permission_callbacks(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
    // Hey friend! Here we register the handler for "Allow Action" button click.
    // It captures the unique permission id and forwards an Approve command.
    window.on_permission_approved(|id| {
        println!("[operon-gui][permission] User approved permission id: {}", id);
        if let Some(cmd_tx) = crate::main_content::input::send::get_active_cmd_tx() {
            tokio::spawn(async move {
                if let Err(e) = cmd_tx.send(operon_rs::SessionCommand::Approve { id: id.to_string() }).await {
                    eprintln!("[operon-gui][permission] Failed to send Approve command: {}", e);
                }
            });
        }
    });

    // Hey friend! Here we register the handler for "Deny" button click.
    // It captures the unique permission id and forwards a Deny command.
    window.on_permission_denied(|id| {
        println!("[operon-gui][permission] User denied permission id: {}", id);
        if let Some(cmd_tx) = crate::main_content::input::send::get_active_cmd_tx() {
            tokio::spawn(async move {
                if let Err(e) = cmd_tx.send(operon_rs::SessionCommand::Deny { id: id.to_string() }).await {
                    eprintln!("[operon-gui][permission] Failed to send Deny command: {}", e);
                }
            });
        }
    });
}
