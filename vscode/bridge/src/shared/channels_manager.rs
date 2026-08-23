//! Permission registry and real-time streaming event broadcaster for Bridge.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::shared::AppState;
use operon_rs::{SessionCommand, SessionEvent};

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

/// Global AppState storage for emitting events to webviews.
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

/// Type alias for the session event hook callback.
pub type ChannelEventHook =
    Arc<dyn Fn(&str, &SessionEvent, &mpsc::Sender<SessionCommand>) + Send + Sync>;

/// Creates a session event hook that streams live agent events to the webview and registers permission requests.
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
