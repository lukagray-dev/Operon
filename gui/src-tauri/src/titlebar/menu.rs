//! Titlebar menu actions: external URLs, project opener, exit.

use std::process::Command;
use tauri::AppHandle;

pub const DOCUMENTATION_URL: &str = "https://github.com/lukagray-dev/Operon/tree/main/docs";
pub const REPORT_BUG_URL: &str = "https://github.com/lukagray-dev/Operon/issues";
pub const FOLLOW_CREATOR_URL: &str = "https://www.instagram.com/lukagray.official/";
pub const REPOSITORY_URL: &str = "https://github.com/lukagray-dev/Operon";

/// Platform-specific URL launcher.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    let normalized = url.trim().to_string();
    if normalized.is_empty() {
        return Err("URL cannot be empty".into());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &normalized])
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&normalized)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&normalized)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn open_documentation() -> Result<(), String> {
    open_external_url(DOCUMENTATION_URL.into()).await
}

#[tauri::command]
pub async fn open_report_bug() -> Result<(), String> {
    open_external_url(REPORT_BUG_URL.into()).await
}

#[tauri::command]
pub async fn open_follow_creator() -> Result<(), String> {
    open_external_url(FOLLOW_CREATOR_URL.into()).await
}

#[tauri::command]
pub async fn open_repository() -> Result<(), String> {
    open_external_url(REPOSITORY_URL.into()).await
}

#[tauri::command]
pub async fn exit_application(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
