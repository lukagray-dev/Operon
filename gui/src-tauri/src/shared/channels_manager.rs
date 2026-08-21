//! Centralized channels service manager for WhatsApp and Telegram integrations.
//!
//! Handles background service lifecycle:
//! - Auto-reconnect on application launch if credentials/tokens exist on disk.
//! - Running live background message processing loops for incoming channel chats.
//! - Restarting services when settings change.
//! - Global permission registry and event streaming hook.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::mpsc;

use operon_rs::{SessionCommand, SessionEvent};
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelPermissionRequestDto {
    pub session_id: String,
    pub id: String,
    pub tool: String,
    pub path: Option<String>,
    pub reason: String,
    pub args_json: String,
}

pub struct PendingPermissionEntry {
    pub session_id: String,
    pub request: ChannelPermissionRequestDto,
    pub cmd_tx: mpsc::Sender<SessionCommand>,
}

/// Thread-safe registry mapping permission_id -> PendingPermissionEntry.
pub static GLOBAL_PERMISSION_REGISTRY: std::sync::Mutex<Option<HashMap<String, PendingPermissionEntry>>> =
    std::sync::Mutex::new(None);

/// Global AppHandle storage for emitting events across channel threads to webviews.
pub static GLOBAL_APP_HANDLE: std::sync::Mutex<Option<tauri::AppHandle>> =
    std::sync::Mutex::new(None);

pub fn set_app_handle(handle: tauri::AppHandle) {
    if let Ok(mut lock) = GLOBAL_APP_HANDLE.lock() {
        *lock = Some(handle);
    }
}

pub fn get_app_handle() -> Option<tauri::AppHandle> {
    if let Ok(lock) = GLOBAL_APP_HANDLE.lock() {
        lock.clone()
    } else {
        None
    }
}

/// Creates a unified event hook that streams live channel events to the Tauri webview and registers permission requests.
pub fn create_channel_event_hook() -> Arc<dyn Fn(&str, &SessionEvent, &mpsc::Sender<SessionCommand>) + Send + Sync> {
    Arc::new(|session_id: &str, event: &SessionEvent, cmd_tx: &mpsc::Sender<SessionCommand>| {
        let app_handle = get_app_handle();

        match event {
            SessionEvent::ApprovalRequired {
                id,
                tool,
                path,
                reason,
                args_json,
            } => {
                let req_dto = ChannelPermissionRequestDto {
                    session_id: session_id.to_string(),
                    id: id.clone(),
                    tool: tool.clone(),
                    path: path.clone(),
                    reason: reason.clone(),
                    args_json: args_json.clone(),
                };

                // Register into global registry so approve_permission / deny_permission can find it
                if let Ok(mut lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
                    let map = lock.get_or_insert_with(HashMap::new);
                    map.insert(
                        id.clone(),
                        PendingPermissionEntry {
                            session_id: session_id.to_string(),
                            request: req_dto.clone(),
                            cmd_tx: cmd_tx.clone(),
                        },
                    );
                }

                // Emit event and native notification to frontend
                if let Some(app) = app_handle {
                    let prefs = crate::settings::prefs::GuiPrefs::load();
                    if prefs.notify_on_permission_request {
                        let desc = if let Some(ref p) = path {
                            format!("Operon requests permission to {} on {}", tool, p)
                        } else {
                            format!("Operon requests permission to {}", tool)
                        };
                        crate::shared::notification::send_desktop_notification(
                            &app,
                            "Operon — Permission Required",
                            &desc,
                        );
                    }
                    let _ = app.emit("channel-permission-request", &req_dto);
                    let _ = app.emit("agent-event", event);
                }
            }
            SessionEvent::ApprovalGranted { .. } | SessionEvent::PermissionDenied { .. } => {
                if let Ok(mut lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
                    if let Some(ref mut map) = *lock {
                        map.retain(|_, entry| entry.session_id != session_id);
                    }
                }
                if let Some(app) = app_handle {
                    let _ = app.emit("channel-permission-resolved", session_id);
                    let _ = app.emit("agent-event", event);
                }
            }
            SessionEvent::Done | SessionEvent::Error { .. } => {
                if let Ok(mut lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
                    if let Some(ref mut map) = *lock {
                        map.retain(|_, entry| entry.session_id != session_id);
                    }
                }
                if let Some(app) = app_handle {
                    let _ = app.emit("channel-permission-resolved", session_id);
                    let _ = app.emit("agent-event", event);
                }
            }
            _ => {
                if let Some(app) = app_handle {
                    let _ = app.emit("agent-event", event);
                }
            }
        }
    })
}

/// Dispatches an approval or denial decision to the pending permission command sender.
pub async fn dispatch_permission_decision(permission_id: &str, is_approve: bool) -> Result<bool, String> {
    let mut sender_opt = None;

    if let Ok(mut lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
        if let Some(ref mut map) = *lock {
            if let Some(entry) = map.remove(permission_id) {
                sender_opt = Some(entry.cmd_tx);
            }
        }
    }

    if let Some(cmd_tx) = sender_opt {
        let cmd = if is_approve {
            SessionCommand::Approve {
                id: permission_id.to_string(),
            }
        } else {
            SessionCommand::Deny {
                id: permission_id.to_string(),
            }
        };
        cmd_tx.send(cmd).await.map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Retrieves all currently pending permission requests across all channels.
pub fn get_all_pending_permissions() -> Vec<ChannelPermissionRequestDto> {
    if let Ok(lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
        if let Some(ref map) = *lock {
            return map.values().map(|entry| entry.request.clone()).collect();
        }
    }
    Vec::new()
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

    let event_hook = create_channel_event_hook();
    let service = WhatsAppService::with_event_hook(client, wa_config, app_config, event_hook);
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

    let event_hook = create_channel_event_hook();
    let service = TelegramService::with_event_hook(client, tg_config, app_config, event_hook);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = service.run().await {
            eprintln!("[operon-gui][telegram-auto] TelegramService exited: {}", e);
        }
    });
}
