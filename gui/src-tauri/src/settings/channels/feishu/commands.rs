//! Feishu / Lark Settings Backend Tauri Commands.
//!
//! Handles:
//! - Loading persisted Feishu App credentials, domain, user ID allowlist, and policy coverage.
//! - Testing App ID & App Secret connectivity via FeishuClient test_auth.
//! - Persisting Feishu configuration and starting background service.

use super::types::{FeishuStateDto, SaveFeishuPayloadDto};
use crate::shared::channels_manager::{
    load_feishu_saved_config, save_feishu_saved_config, FeishuSavedConfig, ACTIVE_FEISHU_CLIENT,
};
use operon_rs::channels::feishu::client::FeishuClient;
use operon_rs::channels::feishu::config::FeishuConfig;
use operon_rs::channels::feishu::types::FeishuDomain;

/// Evaluates whether the given workspace path is covered by security policy.
pub fn evaluate_fs_policy_coverage(path_str: &str, default_path: std::path::PathBuf) -> bool {
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

/// Retrieves current Feishu configuration and connection state.
#[tauri::command]
pub async fn get_feishu_state() -> Result<FeishuStateDto, String> {
    let default_fs_ws = FeishuConfig::default().resolved_workspace_dir();
    let saved = load_feishu_saved_config();

    let is_connected = if let Ok(lock) = ACTIVE_FEISHU_CLIENT.lock() {
        lock.is_some()
    } else {
        false
    };

    let conn_status = if is_connected || (!saved.app_id.trim().is_empty() && !saved.app_secret.trim().is_empty()) {
        "Connected".to_string()
    } else {
        "Disconnected".to_string()
    };

    let is_policy_covered =
        evaluate_fs_policy_coverage(&saved.workspace_dir, default_fs_ws.clone());

    Ok(FeishuStateDto {
        connection_status: conn_status,
        app_id: saved.app_id,
        app_secret: saved.app_secret,
        domain: saved.domain,
        owner_user_id: saved.owner_user_id,
        allowlist: saved.allowlist,
        workspace_dir: saved.workspace_dir,
        resolved_workspace_placeholder: default_fs_ws.to_string_lossy().to_string(),
        is_policy_covered,
    })
}

/// Checks policy coverage for Feishu custom workspace directory.
#[tauri::command]
pub async fn check_feishu_policy_coverage(workspace_dir: String) -> Result<bool, String> {
    let default_ws = FeishuConfig::default().resolved_workspace_dir();
    Ok(evaluate_fs_policy_coverage(&workspace_dir, default_ws))
}

/// Opens native folder picker dialog for Feishu workspace directory.
#[tauri::command]
pub async fn pick_feishu_workspace_dialog() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select Feishu Workspace Directory")
        .pick_folder()
        .await;

    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Tests Feishu App ID & Secret credentials by requesting tenant_access_token and bot info.
#[tauri::command]
pub async fn test_feishu_channel_connection(
    app_id: String,
    app_secret: String,
    domain: String,
) -> Result<String, String> {
    let id = app_id.trim();
    let secret = app_secret.trim();
    if id.is_empty() || secret.is_empty() {
        return Err("Please enter both App ID and App Secret first.".to_string());
    }

    let feishu_domain = if domain.trim().eq_ignore_ascii_case("lark") {
        FeishuDomain::Lark
    } else {
        FeishuDomain::Feishu
    };

    let config = FeishuConfig {
        enabled: true,
        app_id: Some(id.to_string()),
        app_secret: Some(secret.to_string()),
        domain: feishu_domain,
        owner_user_id: None,
        allowlist: Vec::new(),
        workspace_dir: None,
        verification_token: None,
        encrypt_key: None,
    };

    let client = FeishuClient::new(config);
    match client.test_auth().await {
        Ok(info) => Ok(format!("✓ {}", info)),
        Err(e) => Err(format!("Feishu credentials test failed: {:#}", e)),
    }
}

/// Persists Feishu configuration and spawns background service.
#[tauri::command]
pub async fn save_feishu_channel_config(payload: SaveFeishuPayloadDto) -> Result<(), String> {
    let app_id = payload.app_id.trim();
    let app_secret = payload.app_secret.trim();
    if app_id.is_empty() || app_secret.is_empty() {
        return Err("App ID and App Secret are required to enable Feishu channel.".to_string());
    }

    let saved = FeishuSavedConfig {
        enabled: true,
        app_id: app_id.to_string(),
        app_secret: app_secret.to_string(),
        domain: payload.domain.trim().to_string(),
        owner_user_id: payload.owner_user_id.trim().to_string(),
        allowlist: payload.allowlist,
        workspace_dir: payload.workspace_dir.trim().to_string(),
    };

    save_feishu_saved_config(&saved)?;

    println!("[operon-gui][feishu-settings] Saved Feishu configuration.");

    // Start background Feishu service
    crate::shared::channels_manager::start_feishu_channel_if_configured().await;

    Ok(())
}

