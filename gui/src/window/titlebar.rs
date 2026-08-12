//! Callback wiring for the custom Slint titlebar.
//!
//! The Slint markup keeps the visual structure, while this module connects the
//! root component callbacks to the Rust-side state updates and OS actions.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{CloseRequestResponse, ComponentHandle, Weak};

use crate::state::AppState;
use crate::window::{action, menu};

fn with_window(
    handle: &Weak<crate::OperonWindow>,
    action_name: &str,
    f: impl FnOnce(&crate::OperonWindow),
) {
    match handle.upgrade() {
        Some(app) => f(&app),
        None => eprintln!(
            "[operon-gui][window] {action_name} requested, but the window handle was already dropped."
        ),
    }
}

/// Wires every titlebar callback to the actual Rust-side behavior.
pub fn wire_titlebar_callbacks(app: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    // Use a weak handle everywhere so the callback table never keeps the window
    // alive by accident. That is the standard Slint pattern for UI callbacks.
    let app_weak = app.as_weak();

    // If the operating system asks to close the window, we check preferences
    // to decide whether to hide to tray or exit the application cleanly.
    let state_close = Rc::clone(&state);
    app.window().on_close_requested(move || {
        let (close_action, tray_enabled) = {
            let app_state = state_close.borrow();
            (
                app_state.prefs().close_button_action,
                app_state.prefs().minimize_to_tray_enabled,
            )
        };

        if tray_enabled
            && matches!(
                close_action,
                crate::settings::prefs::CloseButtonAction::MinimizeToTray
            )
        {
            eprintln!("[operon-gui][window] Close requested: hiding window to system tray.");
            CloseRequestResponse::HideWindow
        } else {
            eprintln!("[operon-gui][window] Close requested: exiting application.");
            action::exit_application();
            CloseRequestResponse::HideWindow
        }
    });

    app.on_minimize_requested({
        let app_weak = app_weak.clone();
        move || with_window(&app_weak, "minimize", action::minimize_window)
    });

    app.on_maximize_requested({
        let app_weak = app_weak.clone();
        move || {
            with_window(&app_weak, "maximize", |app| {
                let maximized = action::toggle_maximize_window(app);
                app.set_window_maximized(maximized);
            });
        }
    });

    let state_titlebar_close = Rc::clone(&state);
    app.on_close_window_requested({
        let app_weak = app_weak.clone();
        move || {
            let (close_action, tray_enabled) = {
                let app_state = state_titlebar_close.borrow();
                (
                    app_state.prefs().close_button_action,
                    app_state.prefs().minimize_to_tray_enabled,
                )
            };

            if tray_enabled
                && matches!(
                    close_action,
                    crate::settings::prefs::CloseButtonAction::MinimizeToTray
                )
            {
                with_window(&app_weak, "close window", |app| {
                    if let Err(error) = action::close_window(app) {
                        eprintln!("[operon-gui][window] Failed to hide the window: {error:#}");
                    }
                });
            } else {
                action::exit_application();
            }
        }
    });

    app.on_drag_window_requested({
        let app_weak = app_weak.clone();
        move || {
            with_window(&app_weak, "drag window", |app| {
                action::drag_window(app);
            });
        }
    });

    app.on_exit_requested(move || {
        action::exit_application();
    });

    app.on_reload_requested({
        let app_weak = app_weak.clone();
        let state = Rc::clone(&state);
        move || {
            let new_generation = {
                let mut app_state = state.borrow_mut();
                let next = app_state.mark_reload();
                eprintln!(
                    "[operon-gui][view] Reload requested; generation={next}, ui_scale={:.2}",
                    app_state.ui_scale()
                );
                next
            };

            with_window(&app_weak, "reload", |app| {
                app.set_reload_generation(new_generation);
                app.window().request_redraw();
            });
        }
    });

    app.on_zoom_in_requested({
        let app_weak = app_weak.clone();
        let state = Rc::clone(&state);
        move || {
            let new_scale = {
                let mut app_state = state.borrow_mut();
                app_state.zoom_in();
                app_state.ui_scale()
            };

            eprintln!("[operon-gui][view] Zoom in requested; ui_scale={new_scale:.2}");

            with_window(&app_weak, "zoom in", |app| {
                app.set_ui_scale(new_scale);
            });
        }
    });

    app.on_zoom_out_requested({
        let app_weak = app_weak.clone();
        let state = Rc::clone(&state);
        move || {
            let new_scale = {
                let mut app_state = state.borrow_mut();
                app_state.zoom_out();
                app_state.ui_scale()
            };

            eprintln!("[operon-gui][view] Zoom out requested; ui_scale={new_scale:.2}");

            with_window(&app_weak, "zoom out", |app| {
                app.set_ui_scale(new_scale);
            });
        }
    });

    app.on_actual_size_requested({
        let app_weak = app_weak.clone();
        let state = Rc::clone(&state);
        move || {
            let new_scale = {
                let mut app_state = state.borrow_mut();
                app_state.reset_zoom();
                app_state.ui_scale()
            };

            eprintln!("[operon-gui][view] Actual size requested; ui_scale reset to {new_scale:.2}");

            with_window(&app_weak, "actual size", |app| {
                app.set_ui_scale(new_scale);
            });
        }
    });

    app.on_documentation_requested(move || {
        if let Err(error) = menu::open_documentation() {
            eprintln!("[operon-gui][help] Failed to open documentation: {error:#}");
        }
    });

    app.on_report_bug_requested(move || {
        if let Err(error) = menu::open_report_bug() {
            eprintln!("[operon-gui][help] Failed to open issue tracker: {error:#}");
        }
    });

    app.on_follow_creator_requested(move || {
        if let Err(error) = menu::open_follow_creator() {
            eprintln!("[operon-gui][help] Failed to open creator profile: {error:#}");
        }
    });

    app.on_see_repo_requested(move || {
        if let Err(error) = menu::open_repository() {
            eprintln!("[operon-gui][help] Failed to open repository: {error:#}");
        }
    });
}
