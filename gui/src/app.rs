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

    // Modelessly manage settings window instance
    let settings_window_handle: Rc<RefCell<Option<crate::SettingsWindow>>> = Rc::new(RefCell::new(None));
    ui.on_sidebar_settings_clicked({
        let settings_window_handle = Rc::clone(&settings_window_handle);
        move || {
            eprintln!("[operon-gui] Sidebar settings clicked. Launching settings subprocess window.");
            match crate::SettingsWindow::new() {
                Ok(window) => {
                    if let Err(error) = window.show() {
                        eprintln!("[operon-gui] Failed to show settings window: {error:#}");
                    } else {
                        // Storing the new window drops the old handle, which automatically closes any previously open window
                        *settings_window_handle.borrow_mut() = Some(window);
                    }
                }
                Err(error) => {
                    eprintln!("[operon-gui] Failed to construct settings window: {error:#}");
                }
            }
        }
    });

    // `run()` is the easiest Slint entry path for a normal desktop window:
    // it shows the component, runs the event loop, and hides it on shutdown.
    ui.run().context("failed while running the Operon event loop")?;

    eprintln!("[operon-gui] GUI event loop exited cleanly.");
    Ok(())
}
