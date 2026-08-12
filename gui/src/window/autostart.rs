//! System autostart management.
//!
//! Hey friend! This module uses `auto-launch` to configure the application to launch automatically
//! on OS boot (e.g. via Windows Registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).

use auto_launch::AutoLaunchBuilder;

/// Configures OS autostart state for Operon.
pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let auto = AutoLaunchBuilder::new()
        .set_app_name("Operon")
        .set_app_path(&exe.to_string_lossy())
        .build()?;

    if enabled {
        auto.enable()?;
        tracing::info!("[operon-gui][autostart] Autostart enabled via registry.");
    } else {
        auto.disable()?;
        tracing::info!("[operon-gui][autostart] Autostart disabled via registry.");
    }
    Ok(())
}

/// Checks whether OS autostart is currently enabled for Operon.
pub fn is_autostart_enabled() -> anyhow::Result<bool> {
    let exe = std::env::current_exe()?;
    let auto = AutoLaunchBuilder::new()
        .set_app_name("Operon")
        .set_app_path(&exe.to_string_lossy())
        .build()?;

    let enabled = auto.is_enabled()?;
    Ok(enabled)
}
