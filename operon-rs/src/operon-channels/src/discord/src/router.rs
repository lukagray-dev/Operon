// router.rs — Message routing, command parsing, and role resolution for Discord.
//
// Hey friend! This module inspects incoming Discord messages, classifies the sender's role
// (Owner vs External), handles special slash commands like `/new`, pins roles for the lifetime
// of a session, and sends cancellation signals to running sessions when `/new` is received.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::info;

use crate::config::DiscordConfig;
use crate::types::{DiscordChannelId, DiscordMessage, UserId};
use operon_events::SessionCommand;
use operon_policy::CallerRole;

/// Represents an active session record for a Discord user.
#[derive(Debug, Clone)]
pub struct ActiveSession {
    /// Unique session ID formatted as `dc-<hex_timestamp>`.
    pub session_id: String,
    /// Caller role pinned for the lifetime of this session.
    pub role: CallerRole,
    /// Optional command channel sender to cancel in-flight turns.
    pub cmd_tx: Option<mpsc::Sender<SessionCommand>>,
}

/// Outcome of routing an inbound Discord message.
#[derive(Debug, Clone)]
pub enum RouteOutcome {
    /// Inbound message triggers a fresh session reset (`/new`).
    FreshSessionRequested {
        user_id: UserId,
        channel_id: DiscordChannelId,
        new_session_id: String,
        role: CallerRole,
    },
    /// Regular message ready for turn execution.
    ProcessTurn {
        user_id: UserId,
        channel_id: DiscordChannelId,
        session_id: String,
        role: CallerRole,
        is_first_time: bool,
    },
}

/// Router that tracks active session IDs per user, pins caller roles per session,
/// and cancels in-flight turns when `/new` is issued.
pub struct DiscordRouter {
    config: DiscordConfig,
    active_sessions: Arc<Mutex<HashMap<UserId, ActiveSession>>>,
    known_users: Arc<Mutex<HashSet<UserId>>>,
}

impl DiscordRouter {
    /// Creates a new `DiscordRouter` with the given configuration.
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            config,
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            known_users: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Checks if a user ID is considered an owner.
    pub fn is_owner(&self, user_id: &UserId) -> bool {
        self.config.is_owner(user_id)
    }

    /// Evaluates an inbound message and returns a `RouteOutcome`.
    pub async fn route(&self, msg: &DiscordMessage) -> RouteOutcome {
        let user_id = msg.author_id.clone();
        let channel_id = msg.channel_id.clone();
        let trimmed_text = msg.content.trim();

        // ── 1. Check for /new command ───────────────────────────────────────
        if trimmed_text.eq_ignore_ascii_case("/new")
            || trimmed_text.to_lowercase().starts_with("/new ")
        {
            let mut sessions = self.active_sessions.lock().await;

            // If a session is currently active, cancel any in-flight turn!
            let old_role = if let Some(existing) = sessions.get(&user_id) {
                if let Some(ref cmd_tx) = existing.cmd_tx {
                    info!(
                        "Sending SessionCommand::Cancel to running Discord turn for user {}",
                        user_id
                    );
                    let _ = cmd_tx.send(SessionCommand::Cancel).await;
                }
                Some(existing.role)
            } else {
                None
            };

            // Re-evaluate role on /new
            let is_owner = self.is_owner(&user_id);
            let new_role = if is_owner {
                CallerRole::Owner
            } else {
                CallerRole::External
            };

            if let Some(prev_role) = old_role {
                if prev_role != new_role {
                    info!(
                        "Role transition for user {} during /new: {:?} -> {:?}",
                        user_id, prev_role, new_role
                    );
                }
            }

            let new_session_id = generate_session_id();
            sessions.insert(
                user_id.clone(),
                ActiveSession {
                    session_id: new_session_id.clone(),
                    role: new_role,
                    cmd_tx: None,
                },
            );

            return RouteOutcome::FreshSessionRequested {
                user_id,
                channel_id,
                new_session_id,
                role: new_role,
            };
        }

        // ── 2. Determine if first-time user ──────────────────────────────────
        // `HashSet::insert` returns `true` if the user was NOT previously present.
        let is_first_time = self.known_users.lock().await.insert(user_id.clone());

        // ── 3. Resolve active session & pinned role ─────────────────────────
        let mut sessions = self.active_sessions.lock().await;
        let active = match sessions.get(&user_id) {
            Some(existing) => existing.clone(),
            None => {
                let is_owner = self.is_owner(&user_id);
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
                sessions.insert(user_id.clone(), new_active.clone());
                new_active
            }
        };

        RouteOutcome::ProcessTurn {
            user_id,
            channel_id,
            session_id: active.session_id,
            role: active.role,
            is_first_time,
        }
    }

    /// Registers a `cmd_tx` channel for an active session to allow cancelling in-flight turns.
    pub async fn register_cmd_tx(
        &self,
        user_id: &UserId,
        session_id: &str,
        cmd_tx: mpsc::Sender<SessionCommand>,
    ) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(active) = sessions.get_mut(user_id) {
            if active.session_id == session_id {
                active.cmd_tx = Some(cmd_tx);
            }
        }
    }

    /// Unregisters `cmd_tx` when a turn completes.
    pub async fn unregister_cmd_tx(&self, user_id: &UserId, session_id: &str) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(active) = sessions.get_mut(user_id) {
            if active.session_id == session_id {
                active.cmd_tx = None;
            }
        }
    }

    /// Explicitly resets a user's session ID programmatically.
    pub async fn reset_session(&self, user_id: &UserId) -> (String, CallerRole) {
        let mut sessions = self.active_sessions.lock().await;
        let is_owner = self.is_owner(user_id);
        let role = if is_owner {
            CallerRole::Owner
        } else {
            CallerRole::External
        };
        let new_id = generate_session_id();
        sessions.insert(
            user_id.clone(),
            ActiveSession {
                session_id: new_id.clone(),
                role,
                cmd_tx: None,
            },
        );
        (new_id, role)
    }
}

/// Generates a unique session ID based on hex-encoded timestamp prefixed with `dc-`.
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("dc-{:x}", nanos)
}

