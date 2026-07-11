//! Sibling wiring for the settings category navigation sidebar.
//!
//! This module coordinates sidebar category transitions and maintains visual focus.

use std::cell::RefCell;
use std::rc::Rc;
use crate::state::AppState;

/// Binds sidebar event callbacks to application states.
pub fn wire_settings_sidebar(
    _window: &crate::SettingsWindow,
    _state: Rc<RefCell<AppState>>,
) {
    // Left-sidebar navigation relies on Slint two-way binding (<=> active-category).
    // Future expansion (e.g. audit logs, permission counts) can be wired here.
}
