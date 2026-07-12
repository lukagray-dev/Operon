//! Search controller wiring.
//!
//! This module binds search query changes and search cancellation callbacks to
//! dynamic list filtering.

use std::cell::RefCell;
use std::rc::Rc;
use slint::ComponentHandle;

use crate::state::AppState;

/// Wire sidebar search query change and cancel event callbacks.
pub fn wire_search(
    window: &crate::OperonWindow,
    _state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();

    // Callback 1: User typed/changed query in search input field
    window.on_sidebar_search_query_changed({
        let window_weak = window_weak.clone();
        move |_query| {
            if let Some(win) = window_weak.upgrade() {
                super::sidebar::refresh_sidebar(&win);
            }
        }
    });

    // Callback 2: User cancelled/closed search mode
    window.on_sidebar_search_cancelled({
        let window_weak = window_weak.clone();
        move || {
            if let Some(win) = window_weak.upgrade() {
                super::sidebar::refresh_sidebar(&win);
            }
        }
    });
}
