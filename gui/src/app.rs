//! Application bootstrap for the Operon GUI.
//!
//! This is the small layer that owns the long-lived Slint window handle,
//! initializes shared state, and then wires the titlebar callbacks into the
//! Rust-side actions.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Context;
use slint::ComponentHandle;
use slint::winit_030::WinitWindowAccessor;

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
            
            // Check if settings window is already active
            if let Some(existing_window) = settings_window_handle.borrow().as_ref() {
                if let Err(error) = existing_window.show() {
                    eprintln!("[operon-gui] Failed to bring settings window to focus: {error:#}");
                }
                return;
            }
            
            match crate::SettingsWindow::new() {
                Ok(window) => {
                    // Set dark theme preference on the winit window to tell Windows DWM to render 
                    // a dark immersive titlebar and border instead of a bright white/light frame outline.
                    let _ = window.window().with_winit_window(|winit_window: &slint::winit_030::winit::window::Window| {
                        winit_window.set_theme(Some(slint::winit_030::winit::window::Theme::Dark));
                    });

                    // Wire custom titlebar window action callbacks
                    let weak_window = window.as_weak();
                    let weak_handle = Rc::clone(&settings_window_handle);
                    window.on_close_window_requested(move || {
                        eprintln!("[operon-gui] Settings window close button clicked.");
                        if let Some(w) = weak_window.upgrade() {
                            let _ = w.hide();
                        }
                        *weak_handle.borrow_mut() = None;
                    });

                    let weak_window = window.as_weak();
                    window.on_drag_window_requested(move || {
                        if let Some(w) = weak_window.upgrade() {
                            let _ = w.window().with_winit_window(|winit_window| {
                                let _ = winit_window.drag_window();
                            });
                        }
                    });

                    let weak_window = window.as_weak();
                    window.on_minimize_requested(move || {
                        if let Some(w) = weak_window.upgrade() {
                            w.window().set_minimized(true);
                        }
                    });

                    let weak_window = window.as_weak();
                    window.on_maximize_requested(move || {
                        if let Some(w) = weak_window.upgrade() {
                            let next = !w.window().is_maximized();
                            w.window().set_maximized(next);
                            w.set_window_maximized(next);
                        }
                    });

                    window.on_sidebar_toggle_requested(|| {
                        eprintln!("[operon-gui] Settings sidebar toggle requested.");
                    });

                    window.on_github_requested(move || {
                        if let Err(error) = crate::window::menu::open_repository() {
                            eprintln!("[operon-gui][about] Failed to open repository: {error:#}");
                        }
                    });

                    window.on_documentation_requested(move || {
                        if let Err(error) = crate::window::menu::open_documentation() {
                            eprintln!("[operon-gui][about] Failed to open documentation: {error:#}");
                        }
                    });

                    window.on_report_issue_requested(move || {
                        if let Err(error) = crate::window::menu::open_report_bug() {
                            eprintln!("[operon-gui][about] Failed to open issue tracker: {error:#}");
                        }
                    });

                    // Wire dynamic settings categories (e.g. Models config) and navigation
                    crate::settings::wire_settings_window(&window, Rc::clone(&state));
                    
                    if let Err(error) = window.show() {
                        eprintln!("[operon-gui] Failed to show settings window: {error:#}");
                    } else {
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
