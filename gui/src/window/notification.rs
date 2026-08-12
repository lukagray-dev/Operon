//! Desktop Notification controller for Operon GUI.
//!
//! Hey friend! This module provides cross-platform system desktop notifications
//! when agent permission actions require manual user approval.

use std::process::Command;

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
    let body_clone = body.clone();

    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            // On Windows, use PowerShell System.Windows.Forms NotifyIcon balloon tip.
            // Escape single quotes for PowerShell string literals.
            let safe_title = summary_clone.replace('\'', "''");
            let safe_msg = body_clone.replace('\'', "''");
            let ps_script = format!(
                "[reflection.assembly]::loadwithpartialname('System.Windows.Forms') | Out-Null; \
                 $n = New-Object System.Windows.Forms.NotifyIcon; \
                 $n.Icon = [System.Drawing.SystemIcons]::Information; \
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
            let safe_title = summary_clone.replace('\'', "''");
            let safe_msg = body_clone.replace('\'', "''");
            let ps_script = format!(
                "[reflection.assembly]::loadwithpartialname('System.Windows.Forms') | Out-Null; \
                 $n = New-Object System.Windows.Forms.NotifyIcon; \
                 $n.Icon = [System.Drawing.SystemIcons]::Information; \
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
