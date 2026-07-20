//! Helpers for the titlebar's local window controls.
//!
//! These functions are intentionally tiny and direct. They do the window state
//! change, log what happened, and leave callback wiring to the bootstrap layer.

use slint::winit_030::{winit, WinitWindowAccessor};
use slint::ComponentHandle;

/// Minimizes the app window.
pub fn minimize_window(app: &crate::OperonWindow) {
    eprintln!("[operon-gui][window] Minimize requested from the titlebar.");
    app.window().set_minimized(true);
}

/// Toggles the maximized state and returns the new value.
pub fn toggle_maximize_window(app: &crate::OperonWindow) -> bool {
    let window = app.window();
    let previous = window.is_maximized();
    let next = !previous;

    eprintln!("[operon-gui][window] Maximize toggle requested: previous={previous}, next={next}");

    window.set_maximized(next);
    next
}

/// Hides the current window. This is the closest thing Slint exposes to a
/// direct "close this window" action on the component handle.
pub fn close_window(app: &crate::OperonWindow) -> Result<(), slint::PlatformError> {
    eprintln!("[operon-gui][window] Close window requested from the titlebar.");
    app.hide()
}

/// Requests the native backend to start moving the window.
///
/// Slint forwards the custom titlebar's pointer-down event here. The actual
/// moving is delegated to the underlying winit window so the operating system
/// handles it like a native titlebar drag.
pub fn drag_window(app: &crate::OperonWindow) {
    let drag_result = app
        .window()
        .with_winit_window(|winit_window: &winit::window::Window| winit_window.drag_window());

    match drag_result {
        Some(Ok(())) => {
            eprintln!("[operon-gui][window] Window drag started from the titlebar.");
        }
        Some(Err(error)) => {
            eprintln!("[operon-gui][window] Failed to start window drag: {error}");
        }
        None => {
            eprintln!(
                "[operon-gui][window] Failed to start window drag: the current backend does not expose a winit window."
            );
        }
    }
}

/// Terminates the event loop and therefore the application.
pub fn exit_application() {
    eprintln!("[operon-gui][window] Exit requested; terminating the event loop.");

    if let Err(error) = slint::quit_event_loop() {
        eprintln!("[operon-gui][window] Failed to quit the event loop: {error:#}");
    }
}
