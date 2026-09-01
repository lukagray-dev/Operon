//! Discord Settings Backend Tauri Commands.
//!
//! Handles:
//! - Loading persisted Discord bot credentials, user ID allowlist, and policy coverage.
//! - Testing Bot Token connectivity via DiscordClient.
//! - Persisting Discord configuration and starting background Gateway service.

use super::types::{DiscordStateDto, SaveDiscordPayloadDto};
use crate::shared::channels_manager::{
    load_discord_saved_config, save_discord_saved_config, DiscordSavedConfig,
    ACTIVE_DISCORD_CLIENT,
};
use operon_rs::channels::discord::client::DiscordClient;
use operon_rs::channels::discord::config::DiscordConfig;

/// Evaluates whether the given workspace path is covered by security policy.
pub fn evaluate_dc_policy_coverage(path_str: &str, default_path: std::path::PathBuf) -> bool {
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

/// Retrieves current Discord configuration and connection state.
#[tauri::command]
pub async fn get_discord_state() -> Result<DiscordStateDto, String> {
    let default_dc_ws = DiscordConfig::default().resolved_workspace_dir();
    let saved = load_discord_saved_config();

    let is_connected = if let Ok(lock) = ACTIVE_DISCORD_CLIENT.lock() {
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
        evaluate_dc_policy_coverage(&saved.workspace_dir, default_dc_ws.clone());

    Ok(DiscordStateDto {
        connection_status: conn_status,
        bot_token: saved.bot_token,
        owner_user_id: saved.owner_user_id,
        allowlist: saved.allowlist,
        guild_id: saved.guild_id,
        workspace_dir: saved.workspace_dir,
        resolved_workspace_placeholder: default_dc_ws.to_string_lossy().to_string(),
        is_policy_covered,
    })
}

/// Checks policy coverage for Discord custom workspace directory.
#[tauri::command]
pub async fn check_discord_policy_coverage(workspace_dir: String) -> Result<bool, String> {
    let default_ws = DiscordConfig::default().resolved_workspace_dir();
    Ok(evaluate_dc_policy_coverage(&workspace_dir, default_ws))
}

/// Opens native folder picker dialog for Discord workspace directory.
#[tauri::command]
pub async fn pick_discord_workspace_dialog() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select Discord Workspace Directory")
        .pick_folder()
        .await;

    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Tests Discord Bot Token connectivity by calling /users/@me.
#[tauri::command]
pub async fn test_discord_channel_connection(bot_token: String) -> Result<String, String> {
    let token = bot_token.trim();
    if token.is_empty() {
        return Err("Please enter a bot token first.".to_string());
    }

    let config = DiscordConfig {
        enabled: true,
        bot_token: Some(token.to_string()),
        owner_user_id: None,
        allowlist: Vec::new(),
        guild_id: None,
        workspace_dir: None,
    };

    let client = DiscordClient::new(config);
    match client.connect().await {
        Ok(_) => Ok("Bot token is valid and active!".to_string()),
        Err(e) => Err(format!("Token test failed: {:#}", e)),
    }
}

/// Persists Discord configuration and spawns background service.
#[tauri::command]
pub async fn save_discord_channel_config(payload: SaveDiscordPayloadDto) -> Result<(), String> {
    let token = payload.bot_token.trim();
    if token.is_empty() {
        return Err("Bot token is required to enable Discord channel.".to_string());
    }

    let saved = DiscordSavedConfig {
        enabled: true,
        bot_token: token.to_string(),
        owner_user_id: payload.owner_user_id.trim().to_string(),
        allowlist: payload.allowlist,
        guild_id: payload.guild_id.trim().to_string(),
        workspace_dir: payload.workspace_dir.trim().to_string(),
    };

    save_discord_saved_config(&saved)?;

    println!("[operon-gui][discord-settings] Saved Discord configuration.");

    // Start background Discord service
    crate::shared::channels_manager::start_discord_channel_if_configured().await;

    Ok(())
}

