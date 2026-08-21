//! Windows Startup Registry (Autostart) Manager.
//!
//! Controls whether Operon automatically launches upon user login by managing
//! the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Operon` registry key.
//!
//! # Platform Support:
//! - Windows: Uses native `reg.exe` queries and updates.
//! - Non-Windows: Graceful fallback stub for cross-platform portability.

use std::env;
use std::process::Command;

const REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const REG_VALUE_NAME: &str = "Operon";

/// Configures application launch on system startup.
///
/// If `enabled` is true, registers the current executable path in the Windows Run key.
/// If `start_minimized` is also true, appends `--minimized` argument so the app starts hidden in the tray.
/// If `enabled` is false, removes the registry entry.
pub fn set_autostart(enabled: bool, start_minimized: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            let current_exe = env::current_exe().map_err(|e| e.to_string())?;
            let exe_path = current_exe.to_string_lossy().to_string();

            let cmd_val = if start_minimized {
                format!("\"{}\" --minimized", exe_path)
            } else {
                format!("\"{}\"", exe_path)
            };

            // Use Windows built-in reg.exe to add/update the startup registry value
            use std::os::windows::process::CommandExt;
            let mut cmd = Command::new("reg");
            cmd.args(["add", REG_KEY, "/v", REG_VALUE_NAME, "/t", "REG_SZ", "/d", &cmd_val, "/f"]);
            cmd.creation_flags(0x08000000);
            let output = cmd
                .output()
                .map_err(|e| format!("Failed to execute reg.exe: {}", e))?;

            if !output.status.success() {
                let err_msg = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to register startup key: {}", err_msg));
            }
            tracing::info!("Registered autostart: {}", cmd_val);
        } else {
            // Remove registry key if it exists
            use std::os::windows::process::CommandExt;
            let mut cmd = Command::new("reg");
            cmd.args(["delete", REG_KEY, "/v", REG_VALUE_NAME, "/f"]);
            cmd.creation_flags(0x08000000);
            let output = cmd
                .output()
                .map_err(|e| format!("Failed to execute reg.exe: {}", e))?;

            // Note: If key didn't exist, reg.exe returns error code, which is acceptable on disable
            tracing::info!("Unregistered autostart (status: {})", output.status);
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (enabled, start_minimized);
        Ok(())
    }
}

/// Checks if Operon is currently registered in the Windows Run startup registry.
pub fn is_autostart_registered() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("reg");
        cmd.args(["query", REG_KEY, "/v", REG_VALUE_NAME]);
        cmd.creation_flags(0x08000000);
        if let Ok(output) = cmd.output() {
            output.status.success()
        } else {
            false
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autostart_lifecycle() {
        // Test query doesn't crash
        let _ = is_autostart_registered();
    }
}
