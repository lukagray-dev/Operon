//! Send button and message submission event controller.
//!
//! Hey friend! This file registers the GUI event callbacks for the prompt entry submission
//! and execution cancellation. The actual heavy-lifting execution logic, agent runner setup,
//! and event handling loops have been moved to `executor.rs` to maintain clean separation
//! of concerns and a high level of maintainability.

use std::cell::RefCell;
use std::rc::Rc;
use slint::ComponentHandle;

use crate::state::AppState;
use crate::executor;

/// Register message submission callback.
pub fn wire_send(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();
    let app_state = Rc::clone(&state);

    window.on_message_submitted(move |message_text| {
        if let Some(win) = window_weak.upgrade() {
            // Resolve workspace settings on the main thread
            let (session_id, is_new_session) = {
                let mut s = app_state.borrow_mut();
                match s.active_session_id() {
                    Some(id) => (id.to_string(), false),
                    None => {
                        let new_id = format!("{:x}", std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos());
                        s.set_active_session_id(Some(new_id.clone()));
                        (new_id, true)
                    }
                }
            };

            win.set_active_session_id(session_id.clone().into());
            let project_dir = app_state.borrow().current_project_dir().map(String::from);
            executor::submit_prompt(&win, message_text.to_string(), session_id, is_new_session, project_dir);
        }
    });

    window.on_cancel_clicked(move || {
        println!("[operon-gui][send] Stop requested by user");
        let cmd_tx_opt = executor::get_active_cmd_tx();
        if let Some(cmd_tx) = cmd_tx_opt {
            tokio::spawn(async move {
                let _ = cmd_tx.send(operon_rs::SessionCommand::Cancel).await;
            });
        }
    });
}
