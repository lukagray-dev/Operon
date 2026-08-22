//! Centralized channels service manager for WhatsApp and Telegram integrations in Bridge.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::shared::AppState;
use operon_rs::channels::telegram::client::TelegramClient;
use operon_rs::channels::telegram::config::TelegramConfig;
use operon_rs::channels::telegram::service::TelegramService;
use operon_rs::channels::telegram::types::ChatId;
use operon_rs::{SessionCommand, SessionEvent};

use operon_rs::channels::whatsapp::auth::WhatsAppAuth;
use operon_rs::channels::whatsapp::client::WhatsAppClient;
use operon_rs::channels::whatsapp::config::WhatsAppConfig;
use operon_rs::channels::whatsapp::service::WhatsAppService;
use operon_rs::channels::whatsapp::types::ContactId;

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

/// Loads persisted WhatsApp settings from `~/.operon/channels/whatsapp/config.json`.
pub fn load_whatsapp_saved_config() -> WhatsAppSavedConfig {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let path = home
        .join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("config.json");

    if let Ok(content) = std::fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        WhatsAppSavedConfig::default()
    }
}

/// Persists WhatsApp settings to `~/.operon/channels/whatsapp/config.json`.
pub fn save_whatsapp_saved_config(config: &WhatsAppSavedConfig) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".operon").join("channels").join("whatsapp");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = dir.join("config.json");
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Loads persisted Telegram settings from `~/.operon/channels/telegram/config.json`.
pub fn load_telegram_saved_config() -> TelegramSavedConfig {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let path = home
        .join(".operon")
        .join("channels")
        .join("telegram")
        .join("config.json");

    if let Ok(content) = std::fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        TelegramSavedConfig::default()
    }
}

/// Persists Telegram settings to `~/.operon/channels/telegram/config.json`.
pub fn save_telegram_saved_config(config: &TelegramSavedConfig) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".operon").join("channels").join("telegram");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = dir.join("config.json");
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
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
pub static GLOBAL_PERMISSION_REGISTRY: std::sync::Mutex<
    Option<HashMap<String, PendingPermissionEntry>>,
> = std::sync::Mutex::new(None);

/// Global AppState storage for emitting events across channel threads to webviews.
pub static GLOBAL_APP_STATE: std::sync::Mutex<Option<Arc<AppState>>> = std::sync::Mutex::new(None);

pub fn set_app_state(state: Arc<AppState>) {
    if let Ok(mut lock) = GLOBAL_APP_STATE.lock() {
        *lock = Some(state);
    }
}

pub fn get_app_state() -> Option<Arc<AppState>> {
    if let Ok(lock) = GLOBAL_APP_STATE.lock() {
        lock.clone()
    } else {
        None
    }
}

/// Type alias for the unified channel event hook callback.
pub type ChannelEventHook =
    Arc<dyn Fn(&str, &SessionEvent, &mpsc::Sender<SessionCommand>) + Send + Sync>;

/// Creates a unified event hook that streams live channel events to the webview and registers permission requests.
pub fn create_channel_event_hook() -> ChannelEventHook {
    Arc::new(
        |session_id: &str, event: &SessionEvent, cmd_tx: &mpsc::Sender<SessionCommand>| {
            let app_state = get_app_state();

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

                    if let Some(ref state) = app_state {
                        let state_clone = (*state).clone();
                        let dto_clone = req_dto.clone();
                        tokio::spawn(async move {
                            state_clone
                                .emit_event(
                                    "channel-permission-request",
                                    serde_json::to_value(&dto_clone).unwrap_or_default(),
                                )
                                .await;
                        });
                    }
                }
                SessionEvent::ApprovalGranted { .. } | SessionEvent::PermissionDenied { .. } => {
                    if let Ok(mut lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
                        if let Some(ref mut map) = *lock {
                            map.retain(|_, entry| entry.session_id != session_id);
                        }
                    }
                    if let Some(ref state) = app_state {
                        let state_clone = (*state).clone();
                        let sid = session_id.to_string();
                        tokio::spawn(async move {
                            state_clone
                                .emit_event(
                                    "channel-permission-resolved",
                                    serde_json::json!(sid),
                                )
                                .await;
                        });
                    }
                }
                SessionEvent::Done | SessionEvent::Error { .. } => {
                    if let Ok(mut lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
                        if let Some(ref mut map) = *lock {
                            map.retain(|_, entry| entry.session_id != session_id);
                        }
                    }
                    if let Some(ref state) = app_state {
                        let state_clone = (*state).clone();
                        let sid = session_id.to_string();
                        tokio::spawn(async move {
                            state_clone
                                .emit_event(
                                    "channel-permission-resolved",
                                    serde_json::json!(sid),
                                )
                                .await;
                        });
                    }
                }
                _ => {}
            }

            // Broadcast every agent event to VS Code webviews for live token streaming
            if let Some(ref state) = app_state {
                let state_clone = (*state).clone();
                let event_val = serde_json::to_value(event).unwrap_or_default();
                tokio::spawn(async move {
                    state_clone.emit_event("agent-event", event_val).await;
                });
            }
        },
    )
}

/// Dispatches an approval or denial decision to the pending permission command sender.
pub async fn dispatch_permission_decision(
    permission_id: &str,
    approved: bool,
) -> Result<bool, String> {
    let entry = if let Ok(mut lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
        if let Some(ref mut map) = *lock {
            map.remove(permission_id)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(entry) = entry {
        let cmd = if approved {
            SessionCommand::Approve {
                id: permission_id.to_string(),
            }
        } else {
            SessionCommand::Deny {
                id: permission_id.to_string(),
            }
        };

        entry
            .cmd_tx
            .send(cmd)
            .await
            .map_err(|e| format!("Failed to send decision over session channel: {e}"))?;

        if let Some(state) = get_app_state() {
            let state_clone = state.clone();
            let sid = entry.session_id.clone();
            tokio::spawn(async move {
                state_clone
                    .emit_event("channel-permission-resolved", serde_json::json!(sid))
                    .await;
            });
        }

        Ok(true)
    } else {
        Ok(false)
    }
}

/// Returns all pending permissions currently waiting in the registry.
pub fn get_all_pending_permissions() -> Vec<ChannelPermissionRequestDto> {
    if let Ok(lock) = GLOBAL_PERMISSION_REGISTRY.lock() {
        if let Some(ref map) = *lock {
            return map.values().map(|e| e.request.clone()).collect();
        }
    }
    Vec::new()
}

/// Starts WhatsApp channel background service if credentials and enabled configuration exist.
pub async fn start_whatsapp_channel_if_configured() {
    let saved = load_whatsapp_saved_config();
    if !saved.enabled {
        return;
    }

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
        Some(PathBuf::from(saved.workspace_dir.trim()))
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

    let client_clone = client.clone();
    let wa_config_clone = config.clone();

    tokio::spawn(async move {
        if let Err(e) = client_clone.connect().await {
            eprintln!("[operon-bridge][whatsapp-service] Connect failed: {}", e);
        } else if let Ok(app_config) = operon_rs::load() {
            let hook = create_channel_event_hook();
            let service =
                WhatsAppService::with_event_hook(client_clone, wa_config_clone, app_config, hook);
            if let Err(e) = service.run().await {
                eprintln!("[operon-bridge][whatsapp-service] Run failed: {}", e);
            }
        }
    });
}

/// Starts Telegram channel background service if token and enabled configuration exist.
pub async fn start_telegram_channel_if_configured() {
    let saved = load_telegram_saved_config();
    if !saved.enabled || saved.bot_token.trim().is_empty() {
        return;
    }

    let owner_chat = if saved.owner_chat_id.trim().is_empty() {
        None
    } else if let Ok(parsed) = saved.owner_chat_id.trim().parse::<i64>() {
        Some(ChatId::new(parsed))
    } else {
        None
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

    let config = TelegramConfig {
        enabled: true,
        bot_token: Some(saved.bot_token.trim().to_string()),
        owner_chat_id: owner_chat,
        allowlist,
        workspace_dir,
        poll_interval_secs: Some(3),
    };

    let client = Arc::new(TelegramClient::new(config.clone()));
    if let Ok(mut lock) = ACTIVE_TELEGRAM_CLIENT.lock() {
        *lock = Some(client.clone());
    }

    let client_clone = client.clone();

    tokio::spawn(async move {
        if let Err(e) = client_clone.connect().await {
            eprintln!("[operon-bridge][telegram-service] Connect failed: {}", e);
        } else if let Ok(app_config) = operon_rs::load() {
            let hook = create_channel_event_hook();
            let service = TelegramService::with_event_hook(client_clone, config, app_config, hook);
            if let Err(e) = service.run().await {
                eprintln!("[operon-bridge][telegram-service] Run failed: {}", e);
            }
        }
    });
}

/// Initializes all active configured channel background services at application launch.
pub async fn init_channels_on_startup() {
    start_whatsapp_channel_if_configured().await;
    start_telegram_channel_if_configured().await;
}
