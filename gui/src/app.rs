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

    // `run()` is the easiest Slint entry path for a normal desktop window:
    // it shows the component, runs the event loop, and hides it on shutdown.
    ui.run()
        .context("failed while running the Operon event loop")?;

    eprintln!("[operon-gui] GUI event loop exited cleanly.");
    Ok(())
}
