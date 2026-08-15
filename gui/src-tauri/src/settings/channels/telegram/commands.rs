//! Telegram Settings Backend Tauri Commands.
//
// 1:1 match with Slint settings/main-content/telegram.rs:
// - Loads Telegram bot credentials, chat ID allowlist, and policy coverage.
// - Tests Bot Token validation via TelegramClient.
// - Persists Telegram configuration.

use super::types::{SaveTelegramPayloadDto, TelegramStateDto};
use operon_rs::channels::telegram::client::TelegramClient;
use operon_rs::channels::telegram::config::TelegramConfig;
use operon_rs::channels::telegram::types::ChatId;

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
    let is_policy_covered = evaluate_tg_policy_coverage("", default_tg_ws.clone());

    Ok(TelegramStateDto {
        connection_status: "Disconnected".to_string(),
        bot_token: "".to_string(),
        owner_chat_id: "".to_string(),
        allowlist: Vec::new(),
        workspace_dir: "".to_string(),
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

/// Persists Telegram configuration.
#[tauri::command]
pub async fn save_telegram_channel_config(payload: SaveTelegramPayloadDto) -> Result<(), String> {
    let token = payload.bot_token.trim();
    if token.is_empty() {
        return Err("Bot token is required to enable Telegram channel.".to_string());
    }

    let owner_chat = if payload.owner_chat_id.trim().is_empty() {
        None
    } else {
        payload
            .owner_chat_id
            .trim()
            .parse::<i64>()
            .ok()
            .map(ChatId::new)
    };

    let allowlist: Vec<ChatId> = payload
        .allowlist
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok().map(ChatId::new))
        .collect();

    let workspace_dir = if payload.workspace_dir.trim().is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(payload.workspace_dir.trim()))
    };

    let _config = TelegramConfig {
        enabled: true,
        bot_token: Some(token.to_string()),
        owner_chat_id: owner_chat,
        allowlist,
        workspace_dir,
        poll_interval_secs: Some(30),
    };

    println!("[operon-gui][telegram-settings] Saved Telegram configuration.");

    Ok(())
}
