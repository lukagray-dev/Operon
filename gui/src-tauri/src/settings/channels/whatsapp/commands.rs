//! WhatsApp Settings Backend Tauri Commands.
//!
//! Handles:
//! - Loading persisted WhatsApp config, auth state, and policy coverage.
//! - Saving owner mobile number, allowlist, and custom workspace directory.
//! - Real WhatsApp QR code pairing via WhatsAppClient & WhatsAppAuth::render_svg.
//! - Real WhatsApp 8-digit pairing code generation from WhatsApp servers.

use std::sync::Arc;
use std::time::Duration;

use super::types::{SaveWhatsAppPayloadDto, WhatsAppStateDto};
use crate::shared::channels_manager::{
    load_whatsapp_saved_config, save_whatsapp_saved_config, WhatsAppSavedConfig,
    ACTIVE_WHATSAPP_CLIENT,
};
use operon_rs::channels::whatsapp::auth::WhatsAppAuth;
use operon_rs::channels::whatsapp::client::WhatsAppClient;
use operon_rs::channels::whatsapp::config::WhatsAppConfig;
use operon_rs::channels::whatsapp::service::WhatsAppService;
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

    let saved = load_whatsapp_saved_config();
    let auth_checker = WhatsAppAuth::new(default_wa_auth.clone());
    let wa_has_creds = auth_checker.has_credentials();
    let is_client_connected = if let Ok(lock) = ACTIVE_WHATSAPP_CLIENT.lock() {
        lock.is_some()
    } else {
        false
    };

    let wa_status = if wa_has_creds || is_client_connected {
        "Connected".to_string()
    } else {
        "Disconnected".to_string()
    };

    let mut owner_number = saved.owner_number;
    if owner_number.trim().is_empty() && wa_has_creds {
        let session_path = default_wa_auth.join("session.db");
        if let Ok(storage) = operon_rs::channels::whatsapp::RusqliteStore::new(&session_path) {
            use operon_rs::channels::whatsapp::DeviceStore;
            if let Ok(Some(core_device)) = storage.load().await {
                if let Some(pn) = core_device.pn {
                    owner_number = pn.user.to_string();
                }
            }
        }
    }

    let is_policy_covered =
        evaluate_wa_policy_coverage(&saved.workspace_dir, default_wa_ws.clone());

    Ok(WhatsAppStateDto {
        connection_status: wa_status,
        owner_number,
        allowlist: saved.allowlist,
        workspace_dir: saved.workspace_dir,
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

/// Persists WhatsApp configuration and restarts background service if connected.
#[tauri::command]
pub async fn save_whatsapp_channel_config(payload: SaveWhatsAppPayloadDto) -> Result<(), String> {
    let saved = WhatsAppSavedConfig {
        enabled: true,
        owner_number: payload.owner_number.trim().to_string(),
        allowlist: payload.allowlist,
        workspace_dir: payload.workspace_dir.trim().to_string(),
    };

    save_whatsapp_saved_config(&saved)?;

    println!(
        "[operon-gui][whatsapp-settings] Saved WhatsApp config for owner: {}",
        payload.owner_number
    );

    // Restart/start channel service in background
    crate::shared::channels_manager::start_whatsapp_channel_if_configured().await;

    Ok(())
}

/// Generates a real visual QR matrix SVG string from WhatsApp Web for scan pairing.
#[tauri::command]
pub async fn start_whatsapp_qr_pairing() -> Result<String, String> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let default_auth = home
        .join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("auth");

    let saved = load_whatsapp_saved_config();
    let owner_contact = if saved.owner_number.trim().is_empty() {
        None
    } else {
        Some(ContactId::new(saved.owner_number.trim()))
    };

    let allowlist: Vec<ContactId> = saved
        .allowlist
        .iter()
        .map(|s| ContactId::new(s.trim()))
        .collect();

    let workspace_dir = if saved.workspace_dir.trim().is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(saved.workspace_dir.trim()))
    };

    let config = WhatsAppConfig {
        enabled: true,
        owner_number: owner_contact,
        allowlist,
        auth_dir: Some(default_auth),
        workspace_dir,
    };

    let client = Arc::new(WhatsAppClient::new(&config));
    if let Ok(mut lock) = ACTIVE_WHATSAPP_CLIENT.lock() {
        *lock = Some(client.clone());
    }

    let mut qr_rx = client
        .take_qr_receiver()
        .await
        .ok_or_else(|| "QR receiver already active".to_string())?;

    let client_clone = client.clone();
    let wa_config_clone = config.clone();

    // Spawn connect in background
    tokio::spawn(async move {
        if let Err(e) = client_clone.connect().await {
            eprintln!("[operon-gui][whatsapp-qr] Connect error: {}", e);
        } else if let Ok(app_config) = operon_rs::load() {
            let hook = crate::shared::channels_manager::create_channel_event_hook();
            let service =
                WhatsAppService::with_event_hook(client_clone, wa_config_clone, app_config, hook);
            let _ = service.run().await;
        }
    });

    // Await first real QR code state with timeout
    match tokio::time::timeout(Duration::from_secs(15), qr_rx.recv()).await {
        Ok(Some(qr_state)) => {
            let svg = WhatsAppAuth::render_svg(&qr_state.payload)
                .map_err(|e| format!("Failed to render QR SVG: {e}"))?;
            Ok(svg)
        }
        Ok(None) => Err("WhatsApp server closed QR pairing stream".to_string()),
        Err(_) => Err("Timed out waiting for WhatsApp QR code. Please try again.".to_string()),
    }
}

/// Requests a real 8-character pairing code from WhatsApp servers.
#[tauri::command]
pub async fn start_whatsapp_code_pairing(phone_number: String) -> Result<String, String> {
    let clean_phone = phone_number.trim();
    if clean_phone.is_empty() {
        return Err("Please enter your mobile phone number first.".to_string());
    }

    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let default_auth = home
        .join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("auth");

    let saved = load_whatsapp_saved_config();
    let allowlist: Vec<ContactId> = saved
        .allowlist
        .iter()
        .map(|s| ContactId::new(s.trim()))
        .collect();

    let workspace_dir = if saved.workspace_dir.trim().is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(saved.workspace_dir.trim()))
    };

    let config = WhatsAppConfig {
        enabled: true,
        owner_number: Some(ContactId::new(clean_phone)),
        allowlist,
        auth_dir: Some(default_auth),
        workspace_dir,
    };

    let client = Arc::new(WhatsAppClient::new(&config));
    if let Ok(mut lock) = ACTIVE_WHATSAPP_CLIENT.lock() {
        *lock = Some(client.clone());
    }

    let mut code_rx = client
        .take_pairing_code_receiver()
        .await
        .ok_or_else(|| "Pairing code receiver already active".to_string())?;

    let client_clone = client.clone();
    let wa_config_clone = config.clone();

    // Spawn connect in background
    tokio::spawn(async move {
        if let Err(e) = client_clone.connect().await {
            eprintln!("[operon-gui][whatsapp-code] Connect error: {}", e);
        } else if let Ok(app_config) = operon_rs::load() {
            let hook = crate::shared::channels_manager::create_channel_event_hook();
            let service =
                WhatsAppService::with_event_hook(client_clone, wa_config_clone, app_config, hook);
            let _ = service.run().await;
        }
    });

    // Await first real pairing code from server with timeout
    match tokio::time::timeout(Duration::from_secs(15), code_rx.recv()).await {
        Ok(Some(pairing_state)) => Ok(pairing_state.code),
        Ok(None) => Err("WhatsApp server closed pairing code stream".to_string()),
        Err(_) => Err("Timed out waiting for WhatsApp pairing code. Please try again.".to_string()),
    }
}
