//! Settings Window Backend Tauri Commands.
//
// Opens, brings to focus, or closes the standalone Settings WebviewWindow.

use crate::apply_window_dwm_styling;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Opens the standalone Settings window or brings it to the front if already open.
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    // Build the standalone Settings window.
    // The initial and minimum width is set to 860.0px (height: 480.0px) to provide ample horizontal
    // space for the 210px navigation sidebar, settings cards, form controls, and multi-column theme grids.
    let window = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Settings")
        .inner_size(860.0, 480.0)
        .min_inner_size(860.0, 480.0)
        .decorations(false)
        .transparent(false)
        .shadow(false)
        .center()
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    apply_window_dwm_styling(&window);
    let _ = window.show();
    let _ = window.set_focus();

    Ok(())
}

/// Closes the standalone Settings window.
#[tauri::command]
pub async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
