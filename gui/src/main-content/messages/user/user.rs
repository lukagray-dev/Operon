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
pub fn wire_user_messages(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
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

    // Callback 3: Inline edit saved (updates message text and re-parses markdown elements)
    window.on_user_message_edit_saved(move |msg_idx, new_text| {
        if let Some(win) = window_weak3.upgrade() {
            let model = win.get_chat_messages();
            let idx = msg_idx as usize;
            if let Some(mut msg) = model.row_data(idx) {
                msg.text = new_text.clone();
                let parsed_md = markdown::parse_markdown(&new_text);
                msg.markdown_elements = Rc::new(slint::VecModel::from(parsed_md)).into();
                model.set_row_data(idx, msg);
                println!(
                    "[operon-gui][user-message] Saved inline edit for message index {}",
                    msg_idx
                );
            }
        }
    });
}
