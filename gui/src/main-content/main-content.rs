//! Main Content area controller.
//!
//! This module orchestrates the wiring of main content callbacks (prompt input, message actions)
//! and delegates sub-component wiring to child modules.

#[path = "markdown/mod.rs"]
pub mod markdown;

#[path = "input/input.rs"]
pub mod input;

#[path = "messages/user/user.rs"]
pub mod user_messages;

#[path = "messages/assistant/assistant.rs"]
pub mod assistant_messages;

#[path = "messages/loading.rs"]
pub mod loading;

#[path = "reasoning/mod.rs"]
pub mod reasoning;

#[path = "permission/mod.rs"]
pub mod permission;

#[path = "title/title.rs"]
pub mod title;

#[path = "terminal/mod.rs"]
pub mod terminal;

pub mod tools {
    #[path = "cards.rs"]
    pub mod cards;
    #[path = "diff.rs"]
    pub mod diff;
}

use crate::state::AppState;
use std::cell::RefCell;
use std::rc::Rc;

/// Wire all callbacks and update properties inside the main content view.
pub fn wire_main_content(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    // Register main window instance with AppState for live preference propagation
    state.borrow_mut().register_main_window(window);

    // Initialize auto-scroll-stream setting on the chat viewport from user preferences
    window.set_auto_scroll_stream(state.borrow().prefs().auto_scroll_stream);
    window.set_thinking_orb_style(state.borrow().prefs().thinking_orb_style.to_index());

    // Wire prompt input area
    input::wire_input_panel(window, Rc::clone(&state));

    // Wire user and assistant message actions
    user_messages::wire_user_messages(window, Rc::clone(&state));
    assistant_messages::wire_assistant_messages(window, Rc::clone(&state));

    // Wire policy permission approvals
    permission::wire_permission_callbacks(window, Rc::clone(&state));

    // Wire bottom resizable PTY terminal drawer panel
    terminal::wire_terminal(window, Rc::clone(&state));
}
