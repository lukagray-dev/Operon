//! Application bootstrap for the Operon GUI.
//!
//! This is the small layer that owns the long-lived Slint window handle,
//! initializes shared state, and then wires the titlebar callbacks into the
//! Rust-side actions.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Context;
use slint::ComponentHandle;

use crate::state::AppState;
use crate::window::{startup, titlebar};

/// Builds the main window, wires the callbacks, and enters the UI event loop.
pub fn run() -> anyhow::Result<()> {
    eprintln!("[operon-gui] Starting the GUI bootstrap path.");

    let ui = crate::OperonWindow::new().context("failed to create the Operon window")?;
    let state = Rc::new(RefCell::new(AppState::new()));

    // These root-level properties drive the titlebar's zoom state, reload
    // bookkeeping, and maximize button icon before the window is first shown.
    {
        let borrowed_state = state.borrow();
        ui.set_ui_scale(borrowed_state.ui_scale());
        ui.set_reload_generation(borrowed_state.reload_generation());
    }
    ui.set_window_maximized(ui.window().is_maximized());

    startup::apply_startup_geometry(&ui);
    titlebar::wire_titlebar_callbacks(&ui, Rc::clone(&state));

    // Wire left sidebar callbacks and load workspace sessions list
    crate::left_sidebar::wire_left_sidebar(&ui, Rc::clone(&state));

    // Wire main content area callbacks (input panel, message display, etc.)
    crate::main_content::wire_main_content(&ui, Rc::clone(&state));

    // Wire right sidebar Git diff callbacks
    crate::right_sidebar::wire_right_sidebar(&ui, Rc::clone(&state));

    // Register main window with tray module so tray actions can show/restore it
    crate::window::tray::register_main_window(&ui);

    // Initialize tray icon if enabled in preferences
    let (tray_enabled, start_minimized) = {
        let app_state = state.borrow();
        (
            app_state.prefs().minimize_to_tray_enabled,
            app_state.prefs().start_minimized,
        )
    };

    if tray_enabled {
        crate::window::tray::set_tray_active(true);
    }

    // Show the window unless the user has configured the app to start
    // minimized to the system tray.
    if start_minimized && tray_enabled {
        eprintln!("[operon-gui] Starting app minimized to system tray.");
    } else {
        ui.show().context("failed to show the Operon window")?;
    }

    // `run_event_loop_until_quit` behaves like the default event loop, except
    // it keeps running after the last visible window is hidden/closed — which
    // is required for "minimize to tray": `ComponentHandle::run()` (and the
    // plain `run_event_loop()` it wraps) both terminate the moment the last
    // window is hidden, since Slint's default quit-on-last-window-closed
    // policy treats a hidden window the same as a closed one. Tray-based apps
    // must opt out of that policy explicitly and instead rely solely on
    // `slint::quit_event_loop()` (wired in `action::exit_application`) to end
    // the process.
    slint::run_event_loop_until_quit().context("failed while running the Operon event loop")?;

    eprintln!("[operon-gui] GUI event loop exited cleanly.");
    Ok(())
}
