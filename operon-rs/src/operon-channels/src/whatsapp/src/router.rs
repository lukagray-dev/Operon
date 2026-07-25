// router.rs — Message routing, command parsing, and role resolution for WhatsApp.
//
// Hey friend! This module inspects incoming WhatsApp messages, classifies the sender's role
// (Owner vs External), handles special slash commands like `/new`, and manages active session IDs per contact.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use operon_policy::CallerRole;
use crate::config::WhatsAppConfig;
use crate::types::{ContactId, WhatsAppMessage};

/// Outcome of routing an inbound message.
#[derive(Debug, Clone)]
pub enum RouteOutcome {
    /// Inbound message triggers a fresh session reset (`/new`).
    FreshSessionRequested {
        contact: ContactId,
        new_session_id: String,
    },
    /// Regular message ready for turn execution.
    ProcessTurn {
        contact: ContactId,
        session_id: String,
        role: CallerRole,
        is_first_time: bool,
    },
}

/// Router that tracks active session IDs per contact and classifies caller roles.
pub struct WhatsAppRouter {
    config: WhatsAppConfig,
    /// In-memory map from ContactId to current active session ID.
    active_sessions: Arc<Mutex<HashMap<ContactId, String>>>,
    /// In-memory set of known contacts to identify first-time senders.
    known_contacts: Arc<Mutex<Vec<ContactId>>>,
}

impl WhatsAppRouter {
    /// Creates a new `WhatsAppRouter` with the given channel configuration.
    pub fn new(config: WhatsAppConfig) -> Self {
        Self {
            config,
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            known_contacts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Evaluates an inbound message and returns a `RouteOutcome`.
    pub async fn route(&self, msg: &WhatsAppMessage) -> RouteOutcome {
        let contact = msg.sender.clone();
        let trimmed_text = msg.text.trim();

        // ── 1. Check for /new command ───────────────────────────────────────
        if trimmed_text.eq_ignore_ascii_case("/new") || trimmed_text.to_lowercase().starts_with("/new ") {
            let new_session_id = generate_session_id();
            let mut sessions = self.active_sessions.lock().await;
            sessions.insert(contact.clone(), new_session_id.clone());

            return RouteOutcome::FreshSessionRequested {
                contact,
                new_session_id,
            };
        }

        // ── 2. Determine if first-time user ──────────────────────────────────
        let mut known = self.known_contacts.lock().await;
        let is_first_time = !known.contains(&contact);
        if is_first_time {
            known.push(contact.clone());
        }

        // ── 3. Resolve active session ID ─────────────────────────────────────
        let mut sessions = self.active_sessions.lock().await;
        let session_id = match sessions.get(&contact) {
            Some(id) => id.clone(),
            None => {
                let id = generate_session_id();
                sessions.insert(contact.clone(), id.clone());
                id
            }
        };

        // ── 4. Classify role (Owner vs External) ─────────────────────────────
        let is_owner = self.config.is_owner(&contact) || msg.is_self;
        let role = if is_owner {
            CallerRole::Owner
        } else {
            CallerRole::External
        };

        RouteOutcome::ProcessTurn {
            contact,
            session_id,
            role,
            is_first_time,
        }
    }

    /// Explicitly resets a contact's session ID (e.g. programmatically).
    pub async fn reset_session(&self, contact: &ContactId) -> String {
        let new_id = generate_session_id();
        let mut sessions = self.active_sessions.lock().await;
        sessions.insert(contact.clone(), new_id.clone());
        new_id
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
