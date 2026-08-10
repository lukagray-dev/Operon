// router.rs — Message routing, command parsing, and role resolution for Telegram.
//
// Hey friend! This module inspects incoming Telegram messages, classifies the sender's role
// (Owner vs External), handles special slash commands like `/new`, pins roles for the lifetime
// of a session, and sends cancellation signals to running sessions when `/new` is received.

use parking_lot::Mutex as SyncMutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::info;

use crate::config::TelegramConfig;
use crate::types::{ChatId, TelegramMessage};
use operon_events::SessionCommand;
use operon_policy::CallerRole;

/// Represents an active session record for a Telegram chat.
#[derive(Debug, Clone)]
pub struct ActiveSession {
    /// Unique session ID.
    pub session_id: String,
    /// Caller role pinned for the lifetime of this session.
    pub role: CallerRole,
    /// Optional command channel sender to cancel in-flight turns.
    pub cmd_tx: Option<mpsc::Sender<SessionCommand>>,
}

/// Outcome of routing an inbound Telegram message.
#[derive(Debug, Clone)]
pub enum RouteOutcome {
    /// Inbound message triggers a fresh session reset (`/new`).
    FreshSessionRequested {
        chat: ChatId,
        new_session_id: String,
        role: CallerRole,
    },
    /// Regular message ready for turn execution.
    ProcessTurn {
        chat: ChatId,
        session_id: String,
        role: CallerRole,
        is_first_time: bool,
    },
}

/// Router that tracks active session IDs per chat, pins caller roles per session,
/// and cancels in-flight turns when `/new` is issued.
pub struct TelegramRouter {
    config: TelegramConfig,
    /// Resolved owner chat ID for lazy role evaluation.
    owner_chat_id: Arc<SyncMutex<Option<ChatId>>>,
    /// In-memory map from ChatId to current ActiveSession.
    active_sessions: Arc<Mutex<HashMap<ChatId, ActiveSession>>>,
    /// In-memory set of known chats to identify first-time senders in O(1) time.
    known_contacts: Arc<Mutex<HashSet<ChatId>>>,
}

impl TelegramRouter {
    /// Creates a new `TelegramRouter` with the given channel configuration.
    pub fn new(config: TelegramConfig) -> Self {
        let initial_owner = config.owner_chat_id;
        Self {
            config,
            owner_chat_id: Arc::new(SyncMutex::new(initial_owner)),
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            known_contacts: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Checks if a chat ID is considered an owner (via config or lazily resolved owner_chat_id).
    pub fn is_owner(&self, chat: &ChatId) -> bool {
        if self.config.is_owner(chat) {
            return true;
        }
        if let Some(ref owner) = *self.owner_chat_id.lock() {
            if owner == chat {
                return true;
            }
        }
        false
    }

    /// Sets or updates the owner chat ID lazily.
    pub fn set_owner_chat_id(&self, owner: ChatId) {
        *self.owner_chat_id.lock() = Some(owner);
    }

    /// Evaluates an inbound Telegram message and returns a `RouteOutcome`.
    pub async fn route(&self, msg: &TelegramMessage) -> RouteOutcome {
        let chat = msg.sender;
        let trimmed_text = msg.text.trim();

        // ── 1. Check for /new command ───────────────────────────────────────
        if trimmed_text.eq_ignore_ascii_case("/new")
            || trimmed_text.to_lowercase().starts_with("/new ")
        {
            let mut sessions = self.active_sessions.lock().await;

            // If a session is currently active, cancel any in-flight turn!
            let old_role = if let Some(existing) = sessions.get(&chat) {
                if let Some(ref cmd_tx) = existing.cmd_tx {
                    info!(
                        "Sending SessionCommand::Cancel to running turn for chat {}",
                        chat
                    );
                    let _ = cmd_tx.send(SessionCommand::Cancel).await;
                }
                Some(existing.role)
            } else {
                None
            };

            // Re-evaluate role on /new
            let is_owner = self.is_owner(&chat) || msg.is_self;
            let new_role = if is_owner {
                CallerRole::Owner
            } else {
                CallerRole::External
            };

            if let Some(prev_role) = old_role {
                if prev_role != new_role {
                    info!(
                        "Role transition for chat {} during /new: {:?} -> {:?}",
                        chat, prev_role, new_role
                    );
                }
            }

            let new_session_id = generate_session_id();
            sessions.insert(
                chat,
                ActiveSession {
                    session_id: new_session_id.clone(),
                    role: new_role,
                    cmd_tx: None,
                },
            );

            return RouteOutcome::FreshSessionRequested {
                chat,
                new_session_id,
                role: new_role,
            };
        }

        // ── 2. Determine if first-time user ──────────────────────────────────
        // `HashSet::insert` returns `true` if the chat ID was NOT previously present,
        // giving us O(1) detection and insertion in a single atomic step without linear scanning!
        let is_first_time = self.known_contacts.lock().await.insert(chat);

        // ── 3. Resolve active session & pinned role ─────────────────────────
        let mut sessions = self.active_sessions.lock().await;
        let active = match sessions.get(&chat) {
            Some(existing) => existing.clone(),
            None => {
                // First session for this chat: resolve and pin role
                let is_owner = self.is_owner(&chat) || msg.is_self;
                let role = if is_owner {
                    CallerRole::Owner
                } else {
                    CallerRole::External
                };
                let id = generate_session_id();
                let new_active = ActiveSession {
                    session_id: id,
                    role,
                    cmd_tx: None,
                };
                sessions.insert(chat, new_active.clone());
                new_active
            }
        };

        RouteOutcome::ProcessTurn {
            chat,
            session_id: active.session_id,
            role: active.role,
            is_first_time,
        }
    }

    /// Registers a `cmd_tx` channel for an active session to allow cancelling in-flight turns.
    pub async fn register_cmd_tx(
        &self,
        chat: &ChatId,
        session_id: &str,
        cmd_tx: mpsc::Sender<SessionCommand>,
    ) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(active) = sessions.get_mut(chat) {
            if active.session_id == session_id {
                active.cmd_tx = Some(cmd_tx);
            }
        }
    }

    /// Unregisters `cmd_tx` when a turn completes.
    pub async fn unregister_cmd_tx(&self, chat: &ChatId, session_id: &str) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(active) = sessions.get_mut(chat) {
            if active.session_id == session_id {
                active.cmd_tx = None;
            }
        }
    }

    /// Explicitly resets a chat's session ID (e.g. programmatically).
    pub async fn reset_session(&self, chat: &ChatId) -> (String, CallerRole) {
        let mut sessions = self.active_sessions.lock().await;
        let is_owner = self.is_owner(chat);
        let role = if is_owner {
            CallerRole::Owner
        } else {
            CallerRole::External
        };
        let new_id = generate_session_id();
        sessions.insert(
            *chat,
            ActiveSession {
                session_id: new_id.clone(),
                role,
                cmd_tx: None,
            },
        );
        (new_id, role)
    }
}

/// Generates a unique session ID based on hex-encoded timestamp prefixed with "tg-".
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("tg-{:x}", nanos)
}
