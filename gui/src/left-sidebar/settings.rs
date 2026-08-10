//! Settings button click handler and settings window controller.
//!
//! Hey friend! This wires the settings button inside the left sidebar.
//! When clicked, it modelessly constructs and displays the SettingsWindow component.

use crate::state::AppState;
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static SETTINGS_WINDOW_HANDLE: RefCell<Option<crate::SettingsWindow>> = RefCell::new(None);
}

/// Registers the settings button click handler on the main window.
pub fn wire_settings(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let app_state = Rc::clone(&state);

    window.on_sidebar_settings_clicked(move || {
        eprintln!("[operon-gui][settings] Sidebar settings clicked. Launching settings subprocess window.");
        
        // Check if settings window is already active
        let already_active = SETTINGS_WINDOW_HANDLE.with(|handle| {
            if let Some(existing_window) = handle.borrow().as_ref() {
                if let Err(error) = existing_window.show() {
                    eprintln!("[operon-gui][settings] Failed to bring settings window to focus: {error:#}");
                }
                return true;
            }
            false
        });

        if already_active {
            return;
        }
        
        match crate::SettingsWindow::new() {
            Ok(window) => {
                // Set dark theme preference on the winit window to tell Windows DWM to render 
                // a dark immersive titlebar and border instead of a bright white/light frame outline.
                let _ = window.window().with_winit_window(|winit_window: &slint::winit_030::winit::window::Window| {
                    winit_window.set_theme(Some(slint::winit_030::winit::window::Theme::Dark));
                });
                window.window().set_size(slint::PhysicalSize::new(960, 540));

                // Wire custom titlebar window action callbacks
                let weak_window = window.as_weak();
                window.on_close_window_requested(move || {
                    eprintln!("[operon-gui][settings] Settings window close button clicked.");
                    if let Some(w) = weak_window.upgrade() {
                        let _ = w.hide();
                    }
                    SETTINGS_WINDOW_HANDLE.with(|handle| {
                        *handle.borrow_mut() = None;
                    });
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
                    eprintln!("[operon-gui][settings] Settings sidebar toggle requested.");
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
                crate::settings::wire_settings_window(&window, Rc::clone(&app_state));
                
                if let Err(error) = window.show() {
                    eprintln!("[operon-gui][settings] Failed to show settings window: {error:#}");
                } else {
                    SETTINGS_WINDOW_HANDLE.with(|handle| {
                        *handle.borrow_mut() = Some(window);
                    });
                }
            }
            Err(error) => {
                eprintln!("[operon-gui][settings] Failed to construct settings window: {error:#}");
            }
        }
    });
}
