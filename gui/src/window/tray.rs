//! System tray icon integration for Operon GUI.
//!
//! Hey friend! This file manages the system tray icon, menu items ("Show Operon" & "Quit"),
//! and event polling to restore or exit the application.
//!
//! IMPORTANT: `tray-icon` requires an OS message loop to be actively pumping on
//! the same thread the tray icon was created on in order to ever receive click
//! or menu events (on Windows this is a Win32 message loop; Slint's winit
//! backend already runs one as part of its normal event loop). A bare
//! `std::thread::spawn` background loop calling `try_recv()` never sees any
//! events, because no message loop pumps on that thread. We therefore poll via
//! a `slint::Timer` that fires on the main/UI thread, piggybacking on Slint's
//! already-running pump instead of spinning up a separate one.

use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Weak};
use std::cell::RefCell;
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

thread_local! {
    static MAIN_WINDOW_HANDLE: RefCell<Option<Weak<crate::OperonWindow>>> = RefCell::new(None);
    static ACTIVE_TRAY_ICON: RefCell<Option<TrayIcon>> = RefCell::new(None);
    // Held for the lifetime of the poller; dropping a slint::Timer cancels it.
    static EVENT_POLL_TIMER: RefCell<Option<slint::Timer>> = RefCell::new(None);
}

/// Registers the weak main window handle so tray click handlers can show/restore the window.
pub fn register_main_window(window: &crate::OperonWindow) {
    MAIN_WINDOW_HANDLE.with(|handle| {
        *handle.borrow_mut() = Some(window.as_weak());
    });
}

/// Restores and focuses the main application window.
///
/// Safe to call from the main/UI thread directly (as the timer-based poller
/// now does). Also safe to call from another thread, since it marshals
/// through `invoke_from_event_loop` regardless.
pub fn restore_main_window() {
    let weak_opt = MAIN_WINDOW_HANDLE.with(|handle| handle.borrow().clone());
    if let Some(weak) = weak_opt {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = weak.upgrade() {
                if let Err(err) = window.show() {
                    tracing::warn!("[operon-gui][tray] Failed to show main window: {err:#}");
                }
                window.window().set_minimized(false);
                let _ = window.window().with_winit_window(|winit_window| {
                    winit_window.focus_window();
                });
            }
        });
    }
}

/// Constructs and displays the system tray icon.
pub fn build_tray_icon() -> anyhow::Result<TrayIcon> {
    let png_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/tray_icon_32.png"));
    let img = image::load_from_memory(png_bytes)?;
    let rgba = img.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());

    let icon = tray_icon::Icon::from_rgba(rgba.into_raw(), width, height)?;

    let menu = Menu::new();
    let show_item = MenuItem::with_id("show_operon", "Show Operon", true, None);
    let quit_item = MenuItem::with_id("quit_operon", "Quit", true, None);
    menu.append(&show_item)?;
    menu.append(&quit_item)?;

    // Built on the main/UI thread (the same thread Slint's winit event loop
    // pumps on), which is required for the tray icon's helper window to ever
    // receive click/menu messages.
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Operon")
        .with_icon(icon)
        .build()?;

    ensure_event_poller();
    tracing::info!("[operon-gui][tray] System tray icon initialized.");
    Ok(tray)
}

/// Ensures the tray icon is active or destroyed based on user preference.
pub fn set_tray_active(enabled: bool) {
    ACTIVE_TRAY_ICON.with(|cell| {
        let mut slot = cell.borrow_mut();
        if enabled {
            if slot.is_none() {
                match build_tray_icon() {
                    Ok(icon) => *slot = Some(icon),
                    Err(err) => {
                        tracing::error!("[operon-gui][tray] Failed to build tray icon: {err:#}");
                    }
                }
            }
        } else {
            if slot.is_some() {
                *slot = None;
                tracing::info!("[operon-gui][tray] System tray icon removed.");
            }
        }
    });
}

/// Starts a main-thread `slint::Timer` that polls tray menu and click events.
///
/// Must be called only after `build_tray_icon` (i.e. after the tray icon
/// exists on this thread) and while Slint's event loop is available to attach
/// a timer to. Idempotent — safe to call multiple times; only the first call
/// installs the timer.
fn ensure_event_poller() {
    EVENT_POLL_TIMER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_some() {
            return;
        }

        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(100),
            move || {
                poll_tray_events_once();
            },
        );
        *slot = Some(timer);
    });
}

/// Drains any pending tray menu / tray icon click events. Called on the
/// main/UI thread by the `slint::Timer` installed in `ensure_event_poller`.
fn poll_tray_events_once() {
    let menu_receiver = MenuEvent::receiver();
    while let Ok(event) = menu_receiver.try_recv() {
        if event.id == "show_operon" {
            restore_main_window();
        } else if event.id == "quit_operon" {
            let _ = slint::quit_event_loop();
        }
    }

    let tray_receiver = TrayIconEvent::receiver();
    while let Ok(event) = tray_receiver.try_recv() {
        match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                restore_main_window();
            }
            _ => {}
        }
    }
}
