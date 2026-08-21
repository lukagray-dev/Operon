//! System Tray Management for Operon GUI.
//!
//! Provides a system notification area icon with quick actions:
//! - Left Click: Toggle main window visibility (Show / Focus / Hide).
//! - Right Click Context Menu:
//!   - "Open Operon" -> Shows, unminimizes, and focuses the main window.
//!   - "Settings" -> Opens the standalone Settings window.
//!   - Separator
//!   - "Quit Operon" -> Exits the application cleanly.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub const TRAY_ID: &str = "operon-main-tray";

/// Sets up and registers the system tray icon with context menu and click handlers.
pub fn setup_system_tray(app: &AppHandle) -> Result<(), String> {
    // 1. Check if tray is already registered
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }

    // 2. Build Context Menu
    let open_item = MenuItem::with_id(app, "tray_open", "Open Operon", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let settings_item = MenuItem::with_id(app, "tray_settings", "Settings", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let sep_item = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let quit_item = MenuItem::with_id(app, "tray_quit", "Quit Operon", true, None::<&str>)
        .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(app, &[&open_item, &settings_item, &sep_item, &quit_item])
        .map_err(|e| e.to_string())?;

    // 3. Resolve Icon
    let icon = if let Some(default_icon) = app.default_window_icon() {
        default_icon.clone()
    } else {
        return Err("No default application icon available for system tray".to_string());
    };

    // 4. Build Tray Icon
    let _ = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Operon")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray_open" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                }
            }
            "tray_settings" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::settings::open_settings_window(app_handle).await;
                });
            }
            "tray_quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let is_visible = win.is_visible().unwrap_or(false);
                    if is_visible {
                        let _ = win.hide();
                    } else {
                        let _ = win.show();
                        let _ = win.unminimize();
                        let _ = win.set_focus();
                    }
                }
            }
        })
        .build(app)
        .map_err(|e| format!("Failed to create system tray icon: {}", e))?;

    tracing::info!("System tray initialized successfully");
    Ok(())
}

/// Dynamically enables or removes the system tray icon based on user preference.
pub fn update_system_tray(app: &AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        setup_system_tray(app)?;
    } else if let Some(tray) = app.tray_by_id(TRAY_ID) {
        // Destroy the existing tray icon if disabled
        let _ = tray.set_visible(false);
    }
    Ok(())
}
