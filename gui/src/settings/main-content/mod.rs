//! Orchestrator for the settings main content views.

pub mod about;
pub mod appearance;
pub mod extensions;
pub mod general;
pub mod models;
pub mod permissions;
pub mod whatsapp;
pub mod telegram;

use crate::state::AppState;
use std::cell::RefCell;
use std::rc::Rc;

/// Wires all main settings view category panels.
pub fn wire_settings_content(window: &crate::SettingsWindow, state: Rc<RefCell<AppState>>) {
    // Wire models configuration settings panel
    models::wire_models_settings(window, Rc::clone(&state));

    // Wire permissions settings panel
    permissions::wire_permissions_settings(window, Rc::clone(&state));

    // Wire WhatsApp channels settings panel
    whatsapp::wire_whatsapp_settings(window, Rc::clone(&state));

    // Wire Telegram channels settings panel
    telegram::wire_telegram_settings(window, Rc::clone(&state));
}
