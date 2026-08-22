//! About Settings Backend Commands for Bridge.

use super::types::AboutSystemInfoDto;

/// Returns system and application build details.
pub async fn get_about_system_info() -> Result<AboutSystemInfoDto, String> {
    let platform = if cfg!(target_os = "windows") {
        "Windows 10/11".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else {
        std::env::consts::OS.to_string()
    };

    let architecture = std::env::consts::ARCH.to_string();

    Ok(AboutSystemInfoDto {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform,
        architecture,
        ui_toolkit: "VS Code Webview + Stdio RPC Bridge".to_string(),
        compiler: "Rustc 1.78+".to_string(),
    })
}
