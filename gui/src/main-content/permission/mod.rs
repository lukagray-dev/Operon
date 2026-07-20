//! Permission callback wiring and session command triggers.
//!
//! Maps user approval and denial actions from interactive policy checks
//! in the GUI back to the running agent session.

use crate::main_content::assistant_messages::markdown::ParsedMarkdownItem;
use crate::main_content::reasoning::ResponseState;
use crate::state::AppState;
use std::cell::RefCell;
use std::rc::Rc;

/// Registers permission approval and denial callbacks on the Slint window.
///
/// This binds the callbacks exposed by the window's main content area.
pub fn wire_permission_callbacks(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
    // Hey friend! Here we register the handler for "Allow Action" button click.
    // It captures the unique permission id and forwards an Approve command.
    window.on_permission_approved(|id| {
        println!(
            "[operon-gui][permission] User approved permission id: {}",
            id
        );
        if let Some(cmd_tx) = crate::executor::get_active_cmd_tx() {
            tokio::spawn(async move {
                if let Err(e) = cmd_tx
                    .send(operon_rs::SessionCommand::Approve { id: id.to_string() })
                    .await
                {
                    eprintln!(
                        "[operon-gui][permission] Failed to send Approve command: {}",
                        e
                    );
                }
            });
        }
    });

    // Hey friend! Here we register the handler for "Deny" button click.
    // It captures the unique permission id and forwards a Deny command.
    window.on_permission_denied(|id| {
        println!("[operon-gui][permission] User denied permission id: {}", id);
        if let Some(cmd_tx) = crate::executor::get_active_cmd_tx() {
            tokio::spawn(async move {
                if let Err(e) = cmd_tx
                    .send(operon_rs::SessionCommand::Deny { id: id.to_string() })
                    .await
                {
                    eprintln!(
                        "[operon-gui][permission] Failed to send Deny command: {}",
                        e
                    );
                }
            });
        }
    });
}

/// Appends a new pending permission request card.
///
/// Hey friend! This creates a new interactive permission card block when the agent
/// requests approval to execute a tool. It stores the request details (id, path, reason)
/// and registers the card index inside `active_permissions` so we can resolve it later.
pub fn append_approval_required(
    state: &mut ResponseState,
    id: &str,
    tool: &str,
    path: &str,
    reason: &str,
    args_json: &str,
) {
    state.flush_text();
    state.in_thinking = false;

    let pretty_args = if let Ok(val) = serde_json::from_str::<serde_json::Value>(args_json) {
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| args_json.to_string())
    } else {
        args_json.to_string()
    };

    let idx = state.current_blocks.len();
    let mut perm_item = ParsedMarkdownItem::new_default(
        "permission".to_string(),
        String::new(),
        String::new(),
        Vec::new(),
    );
    perm_item.permission_id = id.to_string();
    perm_item.permission_tool = tool.to_string();
    perm_item.permission_path = path.to_string();
    perm_item.permission_reason = reason.to_string();
    perm_item.permission_args = pretty_args;
    perm_item.permission_status = "pending".to_string();

    state.current_blocks.push(perm_item);
    state.active_permissions.insert(id.to_string(), idx);
}

/// Resolves an active permission request card.
///
/// Hey friend! Once the user approves or denies a permission, this function finds the
/// matching card block by searching `active_permissions` and updates its status to
/// "approved" or "denied" in-place.
pub fn append_approval_resolved(state: &mut ResponseState, id: &str, approved: bool) {
    if let Some(&idx) = state.active_permissions.get(id) {
        if let Some(block) = state.current_blocks.get_mut(idx) {
            block.permission_status = if approved {
                "approved".to_string()
            } else {
                "denied".to_string()
            };
        }
    }
}

/// Marks the latest pending permission request as denied when flatly rejected by policy.
///
/// Hey friend! If the running policy denies a tool permission automatically, this searches
/// backwards through the blocks to find the latest pending permission card and denies it.
pub fn append_permission_denied_event(
    state: &mut ResponseState,
    _tool: &str,
    _path: &str,
    _reason: &str,
) {
    for block in state.current_blocks.iter_mut().rev() {
        if block.kind == "permission" && block.permission_status == "pending" {
            block.permission_status = "denied".to_string();
            break;
        }
    }
}
