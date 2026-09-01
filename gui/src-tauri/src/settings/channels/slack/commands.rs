//! Slack Settings Backend Tauri Commands.
//!
//! Handles:
//! - Loading persisted Slack bot credentials, user ID allowlist, and policy coverage.
//! - Testing Bot Token connectivity via SlackClient auth.test.
//! - Persisting Slack configuration and starting background Socket Mode service.

use super::types::{SaveSlackPayloadDto, SlackStateDto};
use crate::shared::channels_manager::{
    load_slack_saved_config, save_slack_saved_config, SlackSavedConfig, ACTIVE_SLACK_CLIENT,
};
use operon_rs::channels::slack::client::SlackClient;
use operon_rs::channels::slack::config::SlackConfig;

/// Evaluates whether the given workspace path is covered by security policy.
pub fn evaluate_sl_policy_coverage(path_str: &str, default_path: std::path::PathBuf) -> bool {
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

/// Retrieves current Slack configuration and connection state.
#[tauri::command]
pub async fn get_slack_state() -> Result<SlackStateDto, String> {
    let default_sl_ws = SlackConfig::default().resolved_workspace_dir();
    let saved = load_slack_saved_config();

    let is_connected = if let Ok(lock) = ACTIVE_SLACK_CLIENT.lock() {
        lock.is_some()
    } else {
        false
    };

    let conn_status = if is_connected || !saved.bot_token.trim().is_empty() {
        "Connected".to_string()
    } else {
        "Disconnected".to_string()
    };

    let is_policy_covered =
        evaluate_sl_policy_coverage(&saved.workspace_dir, default_sl_ws.clone());

    Ok(SlackStateDto {
        connection_status: conn_status,
        bot_token: saved.bot_token,
        app_token: saved.app_token,
        owner_user_id: saved.owner_user_id,
        allowlist: saved.allowlist,
        workspace_dir: saved.workspace_dir,
        resolved_workspace_placeholder: default_sl_ws.to_string_lossy().to_string(),
        is_policy_covered,
    })
}

/// Checks policy coverage for Slack custom workspace directory.
#[tauri::command]
pub async fn check_slack_policy_coverage(workspace_dir: String) -> Result<bool, String> {
    let default_ws = SlackConfig::default().resolved_workspace_dir();
    Ok(evaluate_sl_policy_coverage(&workspace_dir, default_ws))
}

/// Opens native folder picker dialog for Slack workspace directory.
#[tauri::command]
pub async fn pick_slack_workspace_dialog() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select Slack Workspace Directory")
        .pick_folder()
        .await;

    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Tests Slack Bot Token connectivity by calling auth.test.
#[tauri::command]
pub async fn test_slack_channel_connection(bot_token: String) -> Result<String, String> {
    let token = bot_token.trim();
    if token.is_empty() {
        return Err("Please enter a bot token first.".to_string());
    }

    let config = SlackConfig {
        enabled: true,
        bot_token: Some(token.to_string()),
        app_token: None,
        owner_user_id: None,
        allowlist: Vec::new(),
        workspace_dir: None,
    };

    let client = SlackClient::new(config);
    match client.test_auth().await {
        Ok(info) => Ok(format!("✓ {}", info)),
        Err(e) => Err(format!("Token test failed: {:#}", e)),
    }
}

/// Persists Slack configuration and spawns background service.
#[tauri::command]
pub async fn save_slack_channel_config(payload: SaveSlackPayloadDto) -> Result<(), String> {
    let token = payload.bot_token.trim();
    if token.is_empty() {
        return Err("Bot token is required to enable Slack channel.".to_string());
    }

    let saved = SlackSavedConfig {
        enabled: true,
        bot_token: token.to_string(),
        app_token: payload.app_token.trim().to_string(),
        owner_user_id: payload.owner_user_id.trim().to_string(),
        allowlist: payload.allowlist,
        workspace_dir: payload.workspace_dir.trim().to_string(),
    };

    save_slack_saved_config(&saved)?;

    println!("[operon-gui][slack-settings] Saved Slack configuration.");

    // Start background Slack service
    crate::shared::channels_manager::start_slack_channel_if_configured().await;

    Ok(())
}

