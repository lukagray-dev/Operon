//! User message actions controller.
//!
//! Handles copying user message text to clipboard and loading message text back
//! into the input area for editing.

use slint::{ComponentHandle, Model};
use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

pub mod markdown;

/// Wire user message actions.
pub fn wire_user_messages(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak1 = window.as_weak();

    // Callback 1: Copy user message to clipboard
    window.on_user_message_copy_clicked(move |msg_idx| {
        if let Some(win) = window_weak1.upgrade() {
            let model = win.get_chat_messages();
            if let Some(msg) = model.row_data(msg_idx as usize) {
                match arboard::Clipboard::new() {
                    Ok(mut clipboard) => {
                        if let Err(e) = clipboard.set_text(msg.text.to_string()) {
                            eprintln!(
                                "[operon-gui][user-message] Failed to write text to clipboard: {}",
                                e
                            );
                        } else {
                            println!("[operon-gui][user-message] Copied user message to clipboard");
                        }
                    }
                    Err(e) => {
                        eprintln!("[operon-gui][user-message] Failed to open clipboard: {}", e);
                    }
                }
            }
        }
    });

    let window_weak2 = window.as_weak();

    // Callback 2: Edit user message (copies content back into input text field)
    window.on_user_message_edit_clicked(move |msg_idx| {
        if let Some(win) = window_weak2.upgrade() {
            let model = win.get_chat_messages();
            if let Some(msg) = model.row_data(msg_idx as usize) {
                win.set_input_text(msg.text);
                println!(
                    "[operon-gui][user-message] Loaded user message into input field for editing"
                );
            }
        }
    });

    let window_weak3 = window.as_weak();
    let app_state_clone = state.clone();

    // Callback 3: Inline edit saved (updates message text, truncates UI & disk history, and resubmits prompt)
    window.on_user_message_edit_saved(move |msg_idx, new_text| {
        if let Some(win) = window_weak3.upgrade() {
            let model = win.get_chat_messages();
            let idx = msg_idx as usize;

            // 1. Calculate target turn index based on user message count up to msg_idx
            let mut user_msg_count = 0;
            for i in 0..=idx {
                if let Some(msg) = model.row_data(i) {
                    if msg.is_user {
                        user_msg_count += 1;
                    }
                }
            }

            if user_msg_count == 0 {
                return;
            }
            let target_turn_index = user_msg_count - 1;

            // 2. Truncate UI chat_messages model: slice up to msg_idx, update msg_idx with new_text
            let mut msgs: Vec<crate::ChatMessage> = Vec::new();
            for i in 0..=idx {
                if let Some(mut msg) = model.row_data(i) {
                    if i == idx {
                        msg.text = new_text.clone().into();
                        let parsed_md = markdown::parse_markdown(&new_text);
                        msg.markdown_elements = Rc::new(slint::VecModel::from(parsed_md)).into();
                    }
                    msgs.push(msg);
                }
            }
            win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));

            // 3. Cancel any currently active prompt execution
            let cmd_tx_opt = crate::executor::get_active_cmd_tx();
            if let Some(cmd_tx) = cmd_tx_opt {
                tokio::spawn(async move {
                    let _ = cmd_tx.send(operon_rs::SessionCommand::Cancel).await;
                });
            }

            // 4. Truncate persistent session file on disk and resubmit edited prompt
            let state_ref = app_state_clone.borrow();
            let session_id_opt = state_ref.active_session_id().map(String::from);
            let project_dir = state_ref.current_project_dir().map(String::from);
            drop(state_ref);

            if let Some(session_id) = session_id_opt {
                if !session_id.is_empty() {
                    crate::executor::resubmit_edited_prompt(
                        &win,
                        session_id,
                        new_text.to_string(),
                        target_turn_index,
                        project_dir,
                    );
                }
            }
        }
    });
}
