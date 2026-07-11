//! Orchestrator for the settings main content views.

pub mod about;
pub mod appearance;
pub mod channels;
pub mod extensions;
pub mod general;
pub mod models;
pub mod permissions;
pub mod skills;

use std::cell::RefCell;
use std::rc::Rc;
use crate::state::AppState;

/// Wires all main settings view category panels.
pub fn wire_settings_content(
    window: &crate::SettingsWindow,
    state: Rc<RefCell<AppState>>,
) {
    // Wire models configuration settings panel
    models::wire_models_settings(window, Rc::clone(&state));

    // Placeholders for other categories:
    // general::wire_general_settings(window, Rc::clone(&state));
    // appearance::wire_appearance_settings(window, Rc::clone(&state));
}
