//! Window actions: minimize, maximize, close, drag, and sidebar toggle.

use crate::shared::AppState;
use tauri::{State, WebviewWindow};

/// Minimizes the active window.
#[tauri::command]
pub async fn minimize_window(window: WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

/// Toggles between maximized and restored window states.
#[tauri::command]
pub async fn toggle_maximize_window(window: WebviewWindow) -> Result<bool, String> {
    let is_max = window.is_maximized().unwrap_or(false);
    if is_max {
        window.unmaximize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

/// Checks if the window is currently maximized.
#[tauri::command]
pub async fn is_window_maximized(window: WebviewWindow) -> Result<bool, String> {
    Ok(window.is_maximized().unwrap_or(false))
}

/// Closes the application window or hides to tray based on user preferences.
#[tauri::command]
pub async fn close_window(window: WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        let prefs = crate::settings::prefs::GuiPrefs::load();
        if prefs.minimize_to_tray_enabled
            && prefs.close_button_action
                == crate::settings::prefs::CloseButtonAction::MinimizeToTray
        {
            return window.hide().map_err(|e| e.to_string());
        }
    }
    window.close().map_err(|e| e.to_string())
}

/// Begins dragging the window.
#[tauri::command]
pub async fn start_dragging(window: WebviewWindow) -> Result<(), String> {
    let _ = window.start_dragging();
    Ok(())
}

/// Toggles the left sidebar state.
#[tauri::command]
pub async fn toggle_sidebar(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.toggle_sidebar())
}

/// Gets the current sidebar open state.
#[tauri::command]
pub async fn get_sidebar_state(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.is_sidebar_open())
}
