//! User message actions controller.
//!
//! Handles copying user message text to clipboard and loading message text back
//! into the input area for editing.

use std::cell::RefCell;
use std::rc::Rc;
use slint::{ComponentHandle, Model};

use crate::state::AppState;

/// Wire user message actions.
pub fn wire_user_messages(
    window: &crate::OperonWindow,
    _state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();

    // Callback 1: Copy user message to clipboard
    window.on_user_message_copy_clicked(move |msg_idx| {
        if let Some(win) = window_weak.upgrade() {
            let model = win.get_chat_messages();
            if let Some(msg) = model.row_data(msg_idx as usize) {
                match arboard::Clipboard::new() {
                    Ok(mut clipboard) => {
                        if let Err(e) = clipboard.set_text(msg.text.to_string()) {
                            eprintln!("[operon-gui][user-message] Failed to write text to clipboard: {}", e);
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

    let window_weak = window.as_weak();

    // Callback 2: Edit user message (copies content back into input text field)
    window.on_user_message_edit_clicked(move |msg_idx| {
        if let Some(win) = window_weak.upgrade() {
            let model = win.get_chat_messages();
            if let Some(msg) = model.row_data(msg_idx as usize) {
                win.set_input_text(msg.text);
                println!("[operon-gui][user-message] Loaded user message into input field for editing");
            }
        }
    });
}
