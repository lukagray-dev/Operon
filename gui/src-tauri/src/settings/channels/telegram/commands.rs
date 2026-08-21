//! Telegram Settings Backend Tauri Commands.
//!
//! Handles:
//! - Loading persisted Telegram bot credentials, chat ID allowlist, and policy coverage.
//! - Testing Bot Token connectivity via TelegramClient.
//! - Persisting Telegram configuration and starting background polling service.

use super::types::{SaveTelegramPayloadDto, TelegramStateDto};
use crate::shared::channels_manager::{
    load_telegram_saved_config, save_telegram_saved_config, TelegramSavedConfig,
    ACTIVE_TELEGRAM_CLIENT,
};
use operon_rs::channels::telegram::client::TelegramClient;
use operon_rs::channels::telegram::config::TelegramConfig;

/// Evaluates whether the given workspace path is covered by security policy.
pub fn evaluate_tg_policy_coverage(path_str: &str, default_path: std::path::PathBuf) -> bool {
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

/// Retrieves current Telegram configuration and connection state.
#[tauri::command]
pub async fn get_telegram_state() -> Result<TelegramStateDto, String> {
    let default_tg_ws = TelegramConfig::default().resolved_workspace_dir();
    let saved = load_telegram_saved_config();

    let is_connected = if let Ok(lock) = ACTIVE_TELEGRAM_CLIENT.lock() {
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
        evaluate_tg_policy_coverage(&saved.workspace_dir, default_tg_ws.clone());

    Ok(TelegramStateDto {
        connection_status: conn_status,
        bot_token: saved.bot_token,
        owner_chat_id: saved.owner_chat_id,
        allowlist: saved.allowlist,
        workspace_dir: saved.workspace_dir,
        resolved_workspace_placeholder: default_tg_ws.to_string_lossy().to_string(),
        is_policy_covered,
    })
}

/// Checks policy coverage for Telegram custom workspace directory.
#[tauri::command]
pub async fn check_telegram_policy_coverage(workspace_dir: String) -> Result<bool, String> {
    let default_ws = TelegramConfig::default().resolved_workspace_dir();
    Ok(evaluate_tg_policy_coverage(&workspace_dir, default_ws))
}

/// Opens native folder picker dialog for Telegram workspace directory.
#[tauri::command]
pub async fn pick_telegram_workspace_dialog() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select Telegram Workspace Directory")
        .pick_folder()
        .await;

    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Tests Telegram Bot Token connectivity by calling getMe.
#[tauri::command]
pub async fn test_telegram_channel_connection(bot_token: String) -> Result<String, String> {
    let token = bot_token.trim();
    if token.is_empty() {
        return Err("Please enter a bot token first.".to_string());
    }

    let config = TelegramConfig {
        enabled: true,
        bot_token: Some(token.to_string()),
        owner_chat_id: None,
        allowlist: Vec::new(),
        workspace_dir: None,
        poll_interval_secs: Some(30),
    };

    let client = TelegramClient::new(config);
    match client.connect().await {
        Ok(_) => Ok("Bot token is valid and active!".to_string()),
        Err(e) => Err(format!("Token test failed: {:#}", e)),
    }
}

/// Persists Telegram configuration and spawns background service.
#[tauri::command]
pub async fn save_telegram_channel_config(payload: SaveTelegramPayloadDto) -> Result<(), String> {
    let token = payload.bot_token.trim();
    if token.is_empty() {
        return Err("Bot token is required to enable Telegram channel.".to_string());
    }

    let saved = TelegramSavedConfig {
        enabled: true,
        bot_token: token.to_string(),
        owner_chat_id: payload.owner_chat_id.trim().to_string(),
        allowlist: payload.allowlist,
        workspace_dir: payload.workspace_dir.trim().to_string(),
    };

    save_telegram_saved_config(&saved)?;

    println!("[operon-gui][telegram-settings] Saved Telegram configuration.");

    // Start background Telegram service
    crate::shared::channels_manager::start_telegram_channel_if_configured().await;

    Ok(())
}
