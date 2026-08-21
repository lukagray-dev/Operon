// router.rs — Message routing, command parsing, and role resolution for WhatsApp.
//
// Hey friend! This module inspects incoming WhatsApp messages, classifies the sender's role
// (Owner vs External), handles special slash commands like `/new`, pins roles for the lifetime
// of a session, and sends cancellation signals to running sessions when `/new` is received.

use parking_lot::Mutex as SyncMutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::info;

use crate::config::WhatsAppConfig;
use crate::types::{ContactId, WhatsAppMessage};
use operon_events::SessionCommand;
use operon_policy::CallerRole;

/// Represents an active session record for a contact.
#[derive(Debug, Clone)]
pub struct ActiveSession {
    /// Unique session ID.
    pub session_id: String,
    /// Caller role pinned for the lifetime of this session.
    pub role: CallerRole,
    /// Optional command channel sender to cancel in-flight turns.
    pub cmd_tx: Option<mpsc::Sender<SessionCommand>>,
}

/// Outcome of routing an inbound message.
#[derive(Debug, Clone)]
pub enum RouteOutcome {
    /// Inbound message triggers a fresh session reset (`/new`).
    FreshSessionRequested {
        contact: ContactId,
        new_session_id: String,
        role: CallerRole,
    },
    /// Regular message ready for turn execution.
    ProcessTurn {
        contact: ContactId,
        session_id: String,
        role: CallerRole,
        is_first_time: bool,
    },
}

/// Router that tracks active session IDs per contact, pins caller roles per session,
/// and cancels in-flight turns when `/new` is issued.
pub struct WhatsAppRouter {
    config: WhatsAppConfig,
    /// Resolved bot/owner phone number for lazy role evaluation.
    bot_phone: Arc<SyncMutex<Option<ContactId>>>,
    /// In-memory map from ContactId to current ActiveSession.
    active_sessions: Arc<Mutex<HashMap<ContactId, ActiveSession>>>,
    /// In-memory set of known contacts to identify first-time senders in O(1) time.
    known_contacts: Arc<Mutex<HashSet<ContactId>>>,
}

impl WhatsAppRouter {
    /// Creates a new `WhatsAppRouter` with the given channel configuration.
    pub fn new(config: WhatsAppConfig) -> Self {
        let initial_owner = config.owner_number.clone();
        Self {
            config,
            bot_phone: Arc::new(SyncMutex::new(initial_owner)),
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            known_contacts: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Checks if a contact ID is considered an owner (via config or lazily resolved bot_phone).
    pub fn is_owner(&self, contact: &ContactId) -> bool {
        if self.config.is_owner(contact) {
            return true;
        }
        if let Some(ref owner) = *self.bot_phone.lock() {
            if owner == contact {
                return true;
            }
        }
        false
    }

    /// Sets or updates the bot/owner phone number lazily.
    pub fn set_owner_number(&self, owner: ContactId) {
        *self.bot_phone.lock() = Some(owner);
    }

    /// Evaluates an inbound message and returns a `RouteOutcome`.
    pub async fn route(&self, msg: &WhatsAppMessage) -> RouteOutcome {
        let contact = msg.sender.clone();
        let trimmed_text = msg.text.trim();

        // ── 1. Check for /new command ───────────────────────────────────────
        if trimmed_text.eq_ignore_ascii_case("/new")
            || trimmed_text.to_lowercase().starts_with("/new ")
        {
            let mut sessions = self.active_sessions.lock().await;

            // If a session is currently active, cancel any in-flight turn!
            let old_role = if let Some(existing) = sessions.get(&contact) {
                if let Some(ref cmd_tx) = existing.cmd_tx {
                    info!(
                        "Sending SessionCommand::Cancel to running turn for contact {}",
                        contact
                    );
                    let _ = cmd_tx.send(SessionCommand::Cancel).await;
                }
                Some(existing.role)
            } else {
                None
            };

            // Re-evaluate role on /new
            let is_owner = self.is_owner(&contact) || msg.is_self;
            let new_role = if is_owner {
                CallerRole::Owner
            } else {
                CallerRole::External
            };

            if let Some(prev_role) = old_role {
                if prev_role != new_role {
                    info!(
                        "Role transition for contact {} during /new: {:?} -> {:?}",
                        contact, prev_role, new_role
                    );
                }
            }

            let new_session_id = generate_session_id();
            sessions.insert(
                contact.clone(),
                ActiveSession {
                    session_id: new_session_id.clone(),
                    role: new_role,
                    cmd_tx: None,
                },
            );

            return RouteOutcome::FreshSessionRequested {
                contact,
                new_session_id,
                role: new_role,
            };
        }

        // ── 2. Determine if first-time user ──────────────────────────────────
        // Hey newbie friend! `HashSet::insert` returns `true` if the contact was NOT previously present,
        // giving us O(1) detection and insertion in a single atomic step without linear scanning!
        let is_first_time = self.known_contacts.lock().await.insert(contact.clone());

        // ── 3. Resolve active session & pinned role ─────────────────────────
        let mut sessions = self.active_sessions.lock().await;
        let active = match sessions.get(&contact) {
            Some(existing) => existing.clone(),
            None => {
                // First session for this contact: resolve and pin role
                let is_owner = self.is_owner(&contact) || msg.is_self;
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
                sessions.insert(contact.clone(), new_active.clone());
                new_active
            }
        };

        RouteOutcome::ProcessTurn {
            contact,
            session_id: active.session_id,
            role: active.role,
            is_first_time,
        }
    }

    /// Registers a `cmd_tx` channel for an active session to allow cancelling in-flight turns.
    pub async fn register_cmd_tx(
        &self,
        contact: &ContactId,
        session_id: &str,
        cmd_tx: mpsc::Sender<SessionCommand>,
    ) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(active) = sessions.get_mut(contact) {
            if active.session_id == session_id {
                active.cmd_tx = Some(cmd_tx);
            }
        }
    }

    /// Unregisters `cmd_tx` when a turn completes.
    pub async fn unregister_cmd_tx(&self, contact: &ContactId, session_id: &str) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(active) = sessions.get_mut(contact) {
            if active.session_id == session_id {
                active.cmd_tx = None;
            }
        }
    }

    /// Explicitly resets a contact's session ID (e.g. programmatically).
    pub async fn reset_session(&self, contact: &ContactId) -> (String, CallerRole) {
        let mut sessions = self.active_sessions.lock().await;
        let is_owner = self.is_owner(contact);
        let role = if is_owner {
            CallerRole::Owner
        } else {
            CallerRole::External
        };
        let new_id = generate_session_id();
        sessions.insert(
            contact.clone(),
            ActiveSession {
                session_id: new_id.clone(),
                role,
                cmd_tx: None,
            },
        );
        (new_id, role)
    }
}

/// Generates a unique session ID based on hex-encoded timestamp.
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("wa-{:x}", nanos)
}
