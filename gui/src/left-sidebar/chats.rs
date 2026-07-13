//! Projects specific callback wiring and native filesystem dialog interactions.
//!
//! This module separates the general chats setup from project configuration.

use std::cell::RefCell;
use std::rc::Rc;
use rfd::{MessageDialog, MessageLevel, MessageButtons, MessageDialogResult};
use slint::ComponentHandle;

use crate::state::AppState;

/// Register standalone chat setup and selection actions.
pub fn wire_chats(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();

    // Callback 1: Standalone chat clicked
    window.on_sidebar_chat_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |session_id, chat_idx| {
            if let Some(win) = window_weak.upgrade() {
                win.set_active_chat_index(chat_idx);
                win.set_active_project_index(-1);
                win.set_active_conversation_index(-1);
                super::sidebar::load_chat_session(&win, &session_id, None, &app_state);
            }
        }
    });

    // Callback 2: "+" Clicked on sidebar to start a new general session
    window.on_sidebar_new_chat_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move || {
            if let Some(win) = window_weak.upgrade() {
                println!("[operon-gui][sidebar-chats] Creating new standalone chat.");
                {
                    let mut g_state = app_state.borrow_mut();
                    g_state.set_active_session_id(None);
                    g_state.set_current_project_dir(None);
                }
                super::sidebar::clear_sidebar_selection(&win);
                win.set_session_title("New Chat".into());
                win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::default())));
            }
        }
    });

    // Callback 3: Delete a standalone chat session
    window.on_sidebar_delete_chat_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |session_id, chat_idx| {
            let confirmed = MessageDialog::new()
                .set_title("Delete Chat Session")
                .set_description("Are you sure you want to delete this chat session?")
                .set_level(MessageLevel::Warning)
                .set_buttons(MessageButtons::OkCancel)
                .show();

            if confirmed == MessageDialogResult::Ok {
                // Clear state on the main thread first
                let active_id = {
                    let mut g_state = app_state.borrow_mut();
                    if g_state.active_session_id() == Some(&session_id) {
                        g_state.set_active_session_id(None);
                    }
                    g_state.active_session_id().map(String::from)
                };

                let win_weak = window_weak.clone();
                let session_id_clone = session_id.clone();
                
                tokio::spawn(async move {
                    if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
                        let json_path = paths.session_db(&session_id_clone);
                        if json_path.exists() {
                            let _ = std::fs::remove_file(json_path);
                        }
                    }

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = win_weak.upgrade() {
                            if win.get_active_chat_index() == chat_idx {
                                win.set_session_title("New Chat".into());
                            }
                            super::sidebar::clear_sidebar_selection(&win);
                            super::sidebar::refresh_sidebar(&win, active_id);
                        }
                    });
                });
            }
        }
    });
}
