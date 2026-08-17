//! Centralized channels service manager for WhatsApp and Telegram integrations.
//!
//! Handles background service lifecycle:
//! - Auto-reconnect on application launch if credentials/tokens exist on disk.
//! - Running live background message processing loops for incoming channel chats.
//! - Restarting services when settings change.

use std::path::PathBuf;
use std::sync::Arc;

use operon_rs::channels::telegram::client::TelegramClient;
use operon_rs::channels::telegram::config::TelegramConfig;
use operon_rs::channels::telegram::service::TelegramService;
use operon_rs::channels::telegram::types::ChatId;

use operon_rs::channels::whatsapp::auth::WhatsAppAuth;
use operon_rs::channels::whatsapp::client::WhatsAppClient;
use operon_rs::channels::whatsapp::config::WhatsAppConfig;
use operon_rs::channels::whatsapp::service::WhatsAppService;
use operon_rs::channels::whatsapp::types::ContactId;
use operon_rs::channels::whatsapp::{DeviceStore, RusqliteStore};

/// Global handle for active WhatsApp client so pairing and service can coordinate.
pub static ACTIVE_WHATSAPP_CLIENT: std::sync::Mutex<Option<Arc<WhatsAppClient>>> =
    std::sync::Mutex::new(None);

/// Global handle for active Telegram client.
pub static ACTIVE_TELEGRAM_CLIENT: std::sync::Mutex<Option<Arc<TelegramClient>>> =
    std::sync::Mutex::new(None);

/// Persisted configuration JSON format for WhatsApp.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct WhatsAppSavedConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub owner_number: String,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub workspace_dir: String,
}

/// Persisted configuration JSON format for Telegram.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct TelegramSavedConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub owner_chat_id: String,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub workspace_dir: String,
}

/// Resolves the path to `~/.operon/channels/whatsapp/config.json`.
pub fn get_whatsapp_config_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("config.json")
}

/// Resolves the path to `~/.operon/channels/telegram/config.json`.
pub fn get_telegram_config_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".operon")
        .join("channels")
        .join("telegram")
        .join("config.json")
}

/// Loads WhatsApp config from disk.
pub fn load_whatsapp_saved_config() -> WhatsAppSavedConfig {
    let path = get_whatsapp_config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<WhatsAppSavedConfig>(&content) {
                return cfg;
            }
        }
    }
    WhatsAppSavedConfig::default()
}

/// Saves WhatsApp config to disk.
pub fn save_whatsapp_saved_config(cfg: &WhatsAppSavedConfig) -> Result<(), String> {
    let path = get_whatsapp_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json_str = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json_str).map_err(|e| e.to_string())?;
    Ok(())
}

/// Loads Telegram config from disk.
pub fn load_telegram_saved_config() -> TelegramSavedConfig {
    let path = get_telegram_config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<TelegramSavedConfig>(&content) {
                return cfg;
            }
        }
    }
    TelegramSavedConfig::default()
}

/// Saves Telegram config to disk.
pub fn save_telegram_saved_config(cfg: &TelegramSavedConfig) -> Result<(), String> {
    let path = get_telegram_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json_str = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json_str).map_err(|e| e.to_string())?;
    Ok(())
}

/// Spawns background services on application launch if configured.
pub fn auto_start_channels_on_launch() {
    tauri::async_runtime::spawn(async move {
        start_whatsapp_channel_if_configured().await;
        start_telegram_channel_if_configured().await;
    });
}

/// Starts WhatsApp channel background service if credentials exist on disk.
pub async fn start_whatsapp_channel_if_configured() {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let default_auth = home
        .join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("auth");

    let auth_checker = WhatsAppAuth::new(default_auth.clone());
    if !auth_checker.has_credentials() {
        return;
    }

    let saved = load_whatsapp_saved_config();
    let session_path = default_auth.join("session.db");
    let owner_number = if !saved.owner_number.trim().is_empty() {
        Some(ContactId::new(saved.owner_number.trim()))
    } else if let Ok(storage) = RusqliteStore::new(&session_path) {
        if let Ok(Some(core_device)) = storage.load().await {
            core_device.pn.as_ref().map(|jid| ContactId::new(&jid.user))
        } else {
            None
        }
    } else {
        None
    };

    let allowlist: Vec<ContactId> = saved
        .allowlist
        .iter()
        .map(|s| ContactId::new(s.trim()))
        .collect();

    let workspace_dir = if saved.workspace_dir.trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(saved.workspace_dir.trim()))
    };

    let wa_config = WhatsAppConfig {
        enabled: true,
        owner_number,
        allowlist,
        auth_dir: Some(default_auth),
        workspace_dir,
    };

    let client = Arc::new(WhatsAppClient::new(&wa_config));
    if let Ok(mut lock) = ACTIVE_WHATSAPP_CLIENT.lock() {
        *lock = Some(client.clone());
    }

    let app_config = match operon_rs::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("[operon-gui][whatsapp-auto] AppConfig error: {}", e);
            return;
        }
    };

    let service = WhatsAppService::new(client, wa_config, app_config);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = service.run().await {
            eprintln!("[operon-gui][whatsapp-auto] WhatsAppService exited: {}", e);
        }
    });
}

/// Starts Telegram channel background service if bot token is configured.
pub async fn start_telegram_channel_if_configured() {
    let saved = load_telegram_saved_config();
    if saved.bot_token.trim().is_empty() {
        return;
    }

    let owner_chat = if saved.owner_chat_id.trim().is_empty() {
        None
    } else {
        saved
            .owner_chat_id
            .trim()
            .parse::<i64>()
            .ok()
            .map(ChatId::new)
    };

    let allowlist: Vec<ChatId> = saved
        .allowlist
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok().map(ChatId::new))
        .collect();

    let workspace_dir = if saved.workspace_dir.trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(saved.workspace_dir.trim()))
    };

    let tg_config = TelegramConfig {
        enabled: true,
        bot_token: Some(saved.bot_token.trim().to_string()),
        owner_chat_id: owner_chat,
        allowlist,
        workspace_dir,
        poll_interval_secs: Some(30),
    };

    let client = Arc::new(TelegramClient::new(tg_config.clone()));
    if let Ok(mut lock) = ACTIVE_TELEGRAM_CLIENT.lock() {
        *lock = Some(client.clone());
    }

    let app_config = match operon_rs::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("[operon-gui][telegram-auto] AppConfig error: {}", e);
            return;
        }
    };

    let service = TelegramService::new(client, tg_config, app_config);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = service.run().await {
            eprintln!("[operon-gui][telegram-auto] TelegramService exited: {}", e);
        }
    });
}
