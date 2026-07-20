//! Search controller wiring.
//!
//! This module binds search query changes and search cancellation callbacks to
//! dynamic list filtering.

use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

/// Wire sidebar search query change and cancel event callbacks.
pub fn wire_search(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();
    let app_state = Rc::clone(&state);

    // Callback 1: User typed/changed query in search input field
    window.on_sidebar_search_query_changed({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&app_state);
        move |_query| {
            if let Some(win) = window_weak.upgrade() {
                let active_id = app_state.borrow().active_session_id().map(String::from);
                crate::left_sidebar::refresh_sidebar(&win, active_id);
            }
        }
    });

    // Callback 2: User cancelled/closed search mode
    window.on_sidebar_search_cancelled({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&app_state);
        move || {
            if let Some(win) = window_weak.upgrade() {
                let active_id = app_state.borrow().active_session_id().map(String::from);
                crate::left_sidebar::refresh_sidebar(&win, active_id);
            }
        }
    });
}
