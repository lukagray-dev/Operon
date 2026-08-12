//! Orchestrator for the modular Settings window controller.

#[path = "left-sidebar/sidebar.rs"]
pub mod left_sidebar;

#[path = "main-content/mod.rs"]
pub mod main_content;

pub mod prefs;

use crate::state::AppState;
use std::cell::RefCell;
use std::rc::Rc;

/// Initializes all settings controllers and binds them to the given window.
pub fn wire_settings_window(window: &crate::SettingsWindow, state: Rc<RefCell<AppState>>) {
    // Wire the settings navigation sidebar
    left_sidebar::wire_settings_sidebar(window, Rc::clone(&state));

    // Wire the viewport panels on the right
    main_content::wire_settings_content(window, Rc::clone(&state));
}
