//! Left Sidebar New Chat button click handler.
//!
//! Hey friend! This wires the new chat button (the "+" icon on the sidebar).
//! Clicking it resets the active session state in both AppState and Slint,
//! preparing a clean workspace slate for a fresh conversation.

use crate::state::AppState;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

/// Registers the handler for starting a new chat.
pub fn wire_new_chat(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();
    let app_state = Rc::clone(&state);

    window.on_sidebar_new_chat_clicked(move || {
        if let Some(win) = window_weak.upgrade() {
            println!("[operon-gui][new-chat] Creating new standalone chat.");
            {
                let mut g_state = app_state.borrow_mut();
                g_state.set_active_session_id(None);
                g_state.set_current_project_dir(None);
            }
            win.set_active_session_id("".into());
            crate::left_sidebar::clear_sidebar_selection(&win);
            crate::main_content::title::set_session_title(&win, "New Chat");
            win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::default())));

            let app_config = operon_rs::load().ok();
            let context_window = app_config
                .as_ref()
                .map(|c| c.provider.model.context_window)
                .unwrap_or(128_000);
            win.set_context_usage(0.0);
            win.set_tokens_used(0);
            win.set_tokens_total(context_window as i32);
            win.set_context_text(
                crate::main_content::input::context::format_tokens(0, context_window as i32).into(),
            );
        }
    });
}
