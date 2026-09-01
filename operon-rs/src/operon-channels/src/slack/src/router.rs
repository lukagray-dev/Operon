// router.rs — Message routing, command parsing, and role resolution for Slack.
//
// Hey friend! This module inspects incoming Slack messages, classifies the sender's role
// (Owner vs External), handles special slash commands like `/new`, pins roles for the lifetime
// of a session, and sends cancellation signals to running sessions when `/new` is received.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::info;

use crate::config::SlackConfig;
use crate::types::{SlackChannelId, SlackMessage, UserId};
use operon_events::SessionCommand;
use operon_policy::CallerRole;

/// Represents an active session record for a Slack user.
#[derive(Debug, Clone)]
pub struct ActiveSession {
    /// Unique session ID formatted as `sl-<hex_timestamp>`.
    pub session_id: String,
    /// Caller role pinned for the lifetime of this session.
    pub role: CallerRole,
    /// Optional command channel sender to cancel in-flight turns.
    pub cmd_tx: Option<mpsc::Sender<SessionCommand>>,
}

/// Outcome of routing an inbound Slack message.
#[derive(Debug, Clone)]
pub enum RouteOutcome {
    /// Inbound message triggers a fresh session reset (`/new`).
    FreshSessionRequested {
        user_id: UserId,
        channel_id: SlackChannelId,
        thread_ts: Option<String>,
        new_session_id: String,
        role: CallerRole,
    },
    /// Regular message ready for turn execution.
    ProcessTurn {
        user_id: UserId,
        channel_id: SlackChannelId,
        session_id: String,
        thread_ts: Option<String>,
        role: CallerRole,
        is_first_time: bool,
    },
}

/// Router that tracks active session IDs per user, pins caller roles per session,
/// and cancels in-flight turns when `/new` is issued.
pub struct SlackRouter {
    config: SlackConfig,
    active_sessions: Arc<Mutex<HashMap<UserId, ActiveSession>>>,
    known_users: Arc<Mutex<HashSet<UserId>>>,
}

impl SlackRouter {
    /// Creates a new `SlackRouter` with the given configuration.
    pub fn new(config: SlackConfig) -> Self {
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
    pub async fn route(&self, msg: &SlackMessage) -> RouteOutcome {
        let user_id = msg.author_id.clone();
        let channel_id = msg.channel_id.clone();
        let thread_ts = msg.thread_ts.clone();
        let trimmed_text = msg.text.trim();

        // ── 1. Check for /new command ───────────────────────────────────────
        if trimmed_text.eq_ignore_ascii_case("/new")
            || trimmed_text.to_lowercase().starts_with("/new ")
        {
            let mut sessions = self.active_sessions.lock().await;

            // If a session is currently active, cancel any in-flight turn!
            let old_role = if let Some(existing) = sessions.get(&user_id) {
                if let Some(ref cmd_tx) = existing.cmd_tx {
                    info!(
                        "Sending SessionCommand::Cancel to running Slack turn for user {}",
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

            if let Some(r) = old_role {
                if r != new_role {
                    info!(
                        "Slack user {} role changed from {:?} to {:?} across /new boundary",
                        user_id, r, new_role
                    );
                }
            }

            let new_session_id = Self::generate_session_id();
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
                thread_ts,
                new_session_id,
                role: new_role,
            };
        }

        // ── 2. Normal Turn Processing ───────────────────────────────────────
        let mut known = self.known_users.lock().await;
        let is_first_time = known.insert(user_id.clone());

        let mut sessions = self.active_sessions.lock().await;
        let (session_id, role) = if let Some(existing) = sessions.get(&user_id) {
            (existing.session_id.clone(), existing.role)
        } else {
            let is_owner = self.is_owner(&user_id);
            let initial_role = if is_owner {
                CallerRole::Owner
            } else {
                CallerRole::External
            };
            let initial_session_id = Self::generate_session_id();

            sessions.insert(
                user_id.clone(),
                ActiveSession {
                    session_id: initial_session_id.clone(),
                    role: initial_role,
                    cmd_tx: None,
                },
            );

            (initial_session_id, initial_role)
        };

        RouteOutcome::ProcessTurn {
            user_id,
            channel_id,
            session_id,
            thread_ts,
            role,
            is_first_time,
        }
    }

    /// Registers an in-flight turn's `SessionCommand` sender to support cancellation.
    pub async fn register_cmd_tx(
        &self,
        user_id: &UserId,
        session_id: &str,
        cmd_tx: mpsc::Sender<SessionCommand>,
    ) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(session) = sessions.get_mut(user_id) {
            if session.session_id == session_id {
                session.cmd_tx = Some(cmd_tx);
            }
        }
    }

    /// Unregisters a `SessionCommand` sender when turn ends.
    pub async fn unregister_cmd_tx(&self, user_id: &UserId, session_id: &str) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(session) = sessions.get_mut(user_id) {
            if session.session_id == session_id {
                session.cmd_tx = None;
            }
        }
    }

    /// Generates a new unique session ID with prefix `sl-`.
    fn generate_session_id() -> String {
        let epoch_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("sl-{:x}", epoch_millis)
    }
}

