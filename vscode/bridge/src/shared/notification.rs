//! Native Desktop Notification Support for Operon GUI.
//!
//! Hey friend! This module provides cross-platform native desktop notifications
//! using Tauri v2 notification plugin with proper Windows AppUserModelID (AUMID)
//! registration and brand icon resolution so notifications show "Operon" and the Operon logo
//! instead of the host terminal (e.g. Windows Terminal).
//!
//! Settings controlled:
//! - `notify_on_response_complete`: fires when assistant turn finishes.
//! - `notify_on_permission_request`: fires when a tool permission approval is needed.

use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Embedded 128x128 brand logo PNG for native Windows toast notifications.
const EMBEDDED_ICON_PNG: &[u8] = include_bytes!("../../icons/128x128.png");

/// The App User Model ID matching `tauri.conf.json` ("com.operon.desktop").
pub const APP_USER_MODEL_ID: &str = "com.operon.desktop";

/// Sets the Windows Process Explicit AppUserModelID on process startup so Windows
/// attributes toast notifications and taskbar entries to Operon instead of the host terminal.
pub fn set_windows_app_id() {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        let wide_id: Vec<u16> = APP_USER_MODEL_ID
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(wide_id.as_ptr());
        }
    }
}

/// Resolves or extracts the brand icon PNG into `~/.operon/assets/icon.png` for toast notifications.
pub fn get_or_extract_icon_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let assets_dir = home.join(".operon").join("assets");
    let icon_file = assets_dir.join("icon.png");

    if !icon_file.exists() {
        let _ = std::fs::create_dir_all(&assets_dir);
        let _ = std::fs::write(&icon_file, EMBEDDED_ICON_PNG);
    }

    if icon_file.exists() {
        Some(icon_file)
    } else {
        None
    }
}

/// Dispatches a native desktop notification with the specified title, body, and brand icon.
pub fn send_desktop_notification(app: &AppHandle, title: &str, body: &str) {
    let mut builder = app.notification().builder().title(title).body(body);

    if let Some(icon_path) = get_or_extract_icon_path() {
        if let Some(path_str) = icon_path.to_str() {
            builder = builder.icon(path_str);
        }
    }

    if let Err(e) = builder.show() {
        tracing::warn!("Failed to dispatch desktop notification: {}", e);
    }
}
