//! General Settings panel controller.
//!
//! Hey friend! This module handles the logic for the General Settings view,
//! wiring controls for startup autostart, minimize to system tray, and window close behavior.

use crate::settings::prefs::CloseButtonAction;
use crate::state::AppState;
use crate::window::{autostart, tray};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

/// Initializes and binds all interaction callbacks for the General Settings panel.
pub fn wire_general_settings(window: &crate::SettingsWindow, state: Rc<RefCell<AppState>>) {
    // Populate current properties from state / preferences
    {
        let app_state = state.borrow();
        let prefs = app_state.prefs();

        // Sync autostart toggle display with OS truth
        let autostart_display = autostart::is_autostart_enabled().unwrap_or(prefs.autostart_enabled);

        window.set_general_autostart_enabled(autostart_display);
        window.set_general_minimize_to_tray_enabled(prefs.minimize_to_tray_enabled);
        window.set_general_start_minimized(prefs.start_minimized);
        window.set_general_close_button_action(match prefs.close_button_action {
            CloseButtonAction::Exit => 0,
            CloseButtonAction::MinimizeToTray => 1,
        });
    }

    // Callback 1: Autostart toggle
    let window_weak = window.as_weak();
    let state_autostart = Rc::clone(&state);
    window.on_general_autostart_toggled(move |enabled| {
        eprintln!("[operon-gui][general] Autostart toggled: {enabled}");
        if let Err(err) = autostart::set_autostart(enabled) {
            eprintln!("[operon-gui][general] Failed to configure autostart: {err:#}");
            if let Some(w) = window_weak.upgrade() {
                let previous = autostart::is_autostart_enabled().unwrap_or(!enabled);
                w.set_general_autostart_enabled(previous);
            }
            return;
        }

        let mut app_state = state_autostart.borrow_mut();
        app_state.prefs_mut().autostart_enabled = enabled;
        if let Err(err) = app_state.prefs().save() {
            eprintln!("[operon-gui][general] Failed to save prefs after autostart update: {err:#}");
        }
    });

    // Callback 2: Minimize to tray toggle
    let window_weak = window.as_weak();
    let state_tray = Rc::clone(&state);
    window.on_general_minimize_to_tray_toggled(move |enabled| {
        eprintln!("[operon-gui][general] Minimize to tray toggled: {enabled}");

        // Live apply: construct or drop tray icon immediately
        tray::set_tray_active(enabled);

        let mut app_state = state_tray.borrow_mut();
        app_state.prefs_mut().minimize_to_tray_enabled = enabled;

        // If tray is disabled, close button action cannot remain MinimizeToTray
        if !enabled && matches!(app_state.prefs().close_button_action, CloseButtonAction::MinimizeToTray) {
            app_state.prefs_mut().close_button_action = CloseButtonAction::Exit;
            if let Some(w) = window_weak.upgrade() {
                w.set_general_close_button_action(0);
            }
        }

        if let Err(err) = app_state.prefs().save() {
            eprintln!("[operon-gui][general] Failed to save prefs after tray update: {err:#}");
        }
    });

    // Callback 3: Start minimized toggle
    let state_minimized = Rc::clone(&state);
    window.on_general_start_minimized_toggled(move |enabled| {
        eprintln!("[operon-gui][general] Start minimized toggled: {enabled}");
        let mut app_state = state_minimized.borrow_mut();
        app_state.prefs_mut().start_minimized = enabled;
        if let Err(err) = app_state.prefs().save() {
            eprintln!("[operon-gui][general] Failed to save prefs after start_minimized update: {err:#}");
        }
    });

    // Callback 4: Close button action selector
    let state_close = Rc::clone(&state);
    window.on_general_close_action_changed(move |idx| {
        eprintln!("[operon-gui][general] Close button action changed: {idx}");
        let action = match idx {
            1 => CloseButtonAction::MinimizeToTray,
            _ => CloseButtonAction::Exit,
        };

        let mut app_state = state_close.borrow_mut();
        app_state.prefs_mut().close_button_action = action;
        if let Err(err) = app_state.prefs().save() {
            eprintln!("[operon-gui][general] Failed to save prefs after close action update: {err:#}");
        }
    });
}
