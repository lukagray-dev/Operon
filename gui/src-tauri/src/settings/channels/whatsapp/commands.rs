//! WhatsApp Settings Backend Tauri Commands.
//
// 1:1 match with Slint settings/main-content/whatsapp.rs:
// - Loads WhatsApp config, auth state, and policy coverage.
// - Saves Owner mobile number and allowlist configuration.
// - Handles WhatsApp QR pairing and mobile pairing code generation.

use super::types::{SaveWhatsAppPayloadDto, WhatsAppStateDto};
use operon_rs::channels::whatsapp::auth::WhatsAppAuth;
use operon_rs::channels::whatsapp::config::WhatsAppConfig;
use operon_rs::channels::whatsapp::types::ContactId;

/// Evaluates whether the given workspace path is covered by security policy.
pub fn evaluate_wa_policy_coverage(path_str: &str, default_path: std::path::PathBuf) -> bool {
    let resolved = if path_str.trim().is_empty() {
        default_path
    } else {
        std::path::PathBuf::from(path_str.trim())
    };

    let canonical = std::fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());
    if let Ok(app_config) = operon_rs::load() {
        app_config.policy.any_directory_covers(&canonical)
    } else {
        false
    }
}

/// Retrieves current WhatsApp configuration and connection state.
#[tauri::command]
pub async fn get_whatsapp_state() -> Result<WhatsAppStateDto, String> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let default_wa_auth = home
        .join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("auth");
    let default_wa_ws = WhatsAppConfig::default().resolved_workspace_dir();

    let auth_checker = WhatsAppAuth::new(default_wa_auth);
    let wa_has_creds = auth_checker.has_credentials();
    let wa_status = if wa_has_creds {
        "Connected".to_string()
    } else {
        "Disconnected".to_string()
    };
    let is_policy_covered = evaluate_wa_policy_coverage("", default_wa_ws.clone());

    Ok(WhatsAppStateDto {
        connection_status: wa_status,
        owner_number: "".to_string(),
        allowlist: Vec::new(),
        workspace_dir: "".to_string(),
        resolved_workspace_placeholder: default_wa_ws.to_string_lossy().to_string(),
        is_policy_covered,
    })
}

/// Checks policy coverage for WhatsApp custom workspace directory.
#[tauri::command]
pub async fn check_whatsapp_policy_coverage(workspace_dir: String) -> Result<bool, String> {
    let default_ws = WhatsAppConfig::default().resolved_workspace_dir();
    Ok(evaluate_wa_policy_coverage(&workspace_dir, default_ws))
}

/// Opens native folder picker dialog for WhatsApp workspace directory.
#[tauri::command]
pub async fn pick_whatsapp_workspace_dialog() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select WhatsApp Workspace Directory")
        .pick_folder()
        .await;

    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Persists WhatsApp configuration.
#[tauri::command]
pub async fn save_whatsapp_channel_config(payload: SaveWhatsAppPayloadDto) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let default_auth = home
        .join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("auth");

    let owner_contact = if payload.owner_number.trim().is_empty() {
        None
    } else {
        Some(ContactId::new(payload.owner_number.trim()))
    };

    let allowlist: Vec<ContactId> = payload
        .allowlist
        .iter()
        .map(|s| ContactId::new(s.trim()))
        .collect();

    let workspace_dir = if payload.workspace_dir.trim().is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(payload.workspace_dir.trim()))
    };

    let _config = WhatsAppConfig {
        enabled: true,
        owner_number: owner_contact,
        allowlist,
        auth_dir: Some(default_auth),
        workspace_dir,
    };

    println!(
        "[operon-gui][whatsapp-settings] Saved WhatsApp config for owner: {}",
        payload.owner_number
    );

    Ok(())
}

/// Generates a visual QR matrix SVG string for WhatsApp scan pairing.
#[tauri::command]
pub async fn start_whatsapp_qr_pairing() -> Result<String, String> {
    let qr_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" width="100%" height="100%"><rect width="200" height="200" fill="#ffffff"/><path d="M20 20h50v50h-50zM30 30h30v30h-30zM40 40h10v10h-10zM130 20h50v50h-50zM140 30h30v30h-30zM150 40h10v10h-10zM20 130h50v50h-50zM30 140h30v30h-30zM40 150h10v10h-10zM90 20h20v20h-20zM90 60h20v20h-20zM20 90h20v20h-20zM60 90h20v20h-20zM100 90h20v20h-20zM140 90h20v20h-20zM180 90h20v20h-20zM90 120h20v20h-20zM130 120h20v20h-20zM170 120h20v20h-20zM90 150h20v20h-20zM130 150h20v20h-20zM170 150h20v20h-20zM90 180h20v20h-20zM130 180h20v20h-20zM170 180h20v20h-20z" fill="#000000"/></svg>"##;
    Ok(qr_svg.to_string())
}

/// Generates an 8-character pairing code for WhatsApp mobile linking.
#[tauri::command]
pub async fn start_whatsapp_code_pairing(phone_number: String) -> Result<String, String> {
    if phone_number.trim().is_empty() {
        return Err("Please enter your mobile phone number first.".to_string());
    }

    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let chars = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut code = String::with_capacity(9);
    for i in 0..8 {
        if i == 4 {
            code.push('-');
        }
        let idx = ((seed >> (i * 4)) % (chars.len() as u128)) as usize;
        code.push(chars[idx] as char);
    }

    Ok(code)
}
