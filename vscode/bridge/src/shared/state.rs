// ============================================================================
// Shared Application State for Operon VS Code Bridge
// ============================================================================

use crate::rpc::RpcTransport;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Global in-memory UI preferences and state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateDto {
    pub sidebar_open: bool,
    pub ui_scale: f32,
    pub active_session_id: Option<String>,
    pub active_project: Option<String>,
    pub auto_approve: bool,
}

impl Default for AppStateDto {
    fn default() -> Self {
        let initial_auto_approve =
            crate::settings::prefs::GuiPrefs::load().global_auto_approve_default;
        Self {
            sidebar_open: true,
            ui_scale: 1.0,
            active_session_id: None,
            active_project: None,
            auto_approve: initial_auto_approve,
        }
    }
}

pub struct AppState {
    pub sidebar_open: AtomicBool,
    pub state_lock: Mutex<AppStateDto>,
    pub transport: RpcTransport,
}

impl AppState {
    pub fn new(transport: RpcTransport) -> Self {
        Self {
            sidebar_open: AtomicBool::new(true),
            state_lock: Mutex::new(AppStateDto::default()),
            transport,
        }
    }

    pub fn toggle_sidebar(&self) -> bool {
        let current = self.sidebar_open.load(Ordering::SeqCst);
        let new_val = !current;
        self.sidebar_open.store(new_val, Ordering::SeqCst);
        if let Ok(mut lock) = self.state_lock.lock() {
            lock.sidebar_open = new_val;
        }
        new_val
    }

    pub fn is_sidebar_open(&self) -> bool {
        self.sidebar_open.load(Ordering::SeqCst)
    }

    pub async fn emit_event<T: Serialize>(&self, event_name: &str, payload: T) {
        if let Ok(val) = serde_json::to_value(payload) {
            self.transport.send_notification("operon://stream-event", serde_json::json!({
                "event": event_name,
                "payload": val
            })).await;
        }
    }
}
