//! Desktop Notification controller for Operon GUI.
//!
//! Hey friend! This module provides cross-platform system desktop notifications
//! when agent permission actions require manual user approval or responses complete.

use std::process::Command;

/// Embedded application `.ico` icon rasterized at build time from `gui/assets/brand/operon.svg`.
const APP_ICO_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tray_icon.ico"));

/// Ensures the Operon application icon is cached in the OS temporary directory (`%TEMP%/operon_app.ico`) and returns its path.
fn get_app_icon_path() -> Option<std::path::PathBuf> {
    let temp_dir = std::env::temp_dir();
    let ico_path = temp_dir.join("operon_app.ico");
    if !ico_path.exists() {
        if let Err(err) = std::fs::write(&ico_path, APP_ICO_BYTES) {
            tracing::warn!("[operon-gui][notification] Failed to write app icon to {}: {err:#}", ico_path.display());
            return None;
        }
    }
    Some(ico_path)
}

/// Dispatches a native desktop system notification when an agent asks for permission approval.
///
/// Spawns asynchronously in a background thread so execution never blocks the main UI event loop.
pub fn send_permission_asking_notification(action: &str, target: &str) {
    let summary = "Operon: Permission Confirmation Requested";
    let body = if target.is_empty() {
        format!("An agent action requires your manual approval: {action}")
    } else {
        format!("An agent action requires your manual approval: {action} on {target}")
    };

    let summary_clone = summary.to_string();
    let body_clone = body;

    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            let icon_script = if let Some(ico_path) = get_app_icon_path() {
                let safe_ico_path = ico_path.to_string_lossy().replace('\'', "''");
                format!(
                    "if (Test-Path '{safe_ico_path}') {{ \
                         $n.Icon = New-Object System.Drawing.Icon('{safe_ico_path}'); \
                     }} else {{ \
                         $n.Icon = [System.Drawing.SystemIcons]::Information; \
                     }}"
                )
            } else {
                "$n.Icon = [System.Drawing.SystemIcons]::Information;".to_string()
            };

            // On Windows, use PowerShell System.Windows.Forms NotifyIcon balloon tip with custom app icon.
            let safe_title = summary_clone.replace('\'', "''");
            let safe_msg = body_clone.replace('\'', "''");
            let ps_script = format!(
                "[reflection.assembly]::loadwithpartialname('System.Windows.Forms') | Out-Null; \
                 [reflection.assembly]::loadwithpartialname('System.Drawing') | Out-Null; \
                 $n = New-Object System.Windows.Forms.NotifyIcon; \
                 {icon_script} \
                 $n.Visible = $true; \
                 $n.ShowBalloonTip(5000, '{safe_title}', '{safe_msg}', [System.Windows.Forms.ToolTipIcon]::Info); \
                 Start-Sleep -s 6; \
                 $n.Dispose()"
            );

            let _ = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
                .output();
        }

        #[cfg(target_os = "macos")]
        {
            let safe_title = summary_clone.replace('"', "\\\"");
            let safe_msg = body_clone.replace('"', "\\\"");
            let script = format!("display notification \"{safe_msg}\" with title \"{safe_title}\"");
            let _ = Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output();
        }

        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("notify-send")
                .arg(&summary_clone)
                .arg(&body_clone)
                .output();
        }
    });
}

/// Dispatches a native desktop system notification when an agent response turn finishes.
///
/// Spawns asynchronously in a background thread so execution never blocks the main UI event loop.
pub fn send_response_complete_notification(session_title: &str) {
    let summary = "Operon: Response Complete";
    let body = if session_title.is_empty() || session_title == "New Session" {
        "Agent finished generating response".to_string()
    } else {
        format!("Agent finished generating response for '{session_title}'")
    };

    let summary_clone = summary.to_string();
    let body_clone = body;

    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            let icon_script = if let Some(ico_path) = get_app_icon_path() {
                let safe_ico_path = ico_path.to_string_lossy().replace('\'', "''");
                format!(
                    "if (Test-Path '{safe_ico_path}') {{ \
                         $n.Icon = New-Object System.Drawing.Icon('{safe_ico_path}'); \
                     }} else {{ \
                         $n.Icon = [System.Drawing.SystemIcons]::Information; \
                     }}"
                )
            } else {
                "$n.Icon = [System.Drawing.SystemIcons]::Information;".to_string()
            };

            let safe_title = summary_clone.replace('\'', "''");
            let safe_msg = body_clone.replace('\'', "''");
            let ps_script = format!(
                "[reflection.assembly]::loadwithpartialname('System.Windows.Forms') | Out-Null; \
                 [reflection.assembly]::loadwithpartialname('System.Drawing') | Out-Null; \
                 $n = New-Object System.Windows.Forms.NotifyIcon; \
                 {icon_script} \
                 $n.Visible = $true; \
                 $n.ShowBalloonTip(5000, '{safe_title}', '{safe_msg}', [System.Windows.Forms.ToolTipIcon]::Info); \
                 Start-Sleep -s 6; \
                 $n.Dispose()"
            );

            let _ = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
                .output();
        }

        #[cfg(target_os = "macos")]
        {
            let safe_title = summary_clone.replace('"', "\\\"");
            let safe_msg = body_clone.replace('"', "\\\"");
            let script = format!("display notification \"{safe_msg}\" with title \"{safe_title}\"");
            let _ = Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output();
        }

        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("notify-send")
                .arg(&summary_clone)
                .arg(&body_clone)
                .output();
        }
    });
}
