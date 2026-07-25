// config.rs — Configuration management for operon-channels-whatsapp.
//
// Hey friend! This module handles settings for the WhatsApp channel, including the main owner
// phone number, allowed contact numbers, auto-reply behavior, and authentication directories.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::types::ContactId;

/// Configuration parameters for WhatsApp channel integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    /// Is the WhatsApp channel enabled.
    pub enabled: bool,
    /// Main owner phone number connected during QR scan setup.
    pub owner_number: Option<ContactId>,
    /// List of allowed contact phone numbers granted `Role::Owner` privileges.
    pub allowlist: Vec<ContactId>,
    /// Custom path for persistent authentication credentials directory.
    /// Defaults to `~/.operon/channels/whatsapp/auth/`.
    pub auth_dir: Option<PathBuf>,
}

impl Default for WhatsAppConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            owner_number: None,
            allowlist: Vec::new(),
            auth_dir: None,
        }
    }
}

impl WhatsAppConfig {
    /// Returns the resolved directory path for auth credential storage.
    pub fn resolved_auth_dir(&self) -> PathBuf {
        if let Some(ref path) = self.auth_dir {
            path.clone()
        } else if let Some(home) = dirs::home_dir() {
            home.join(".operon").join("channels").join("whatsapp").join("auth")
        } else {
            PathBuf::from(".operon/channels/whatsapp/auth")
        }
    }

    /// Checks if a given contact ID is considered an Owner (main number or allowlisted).
    pub fn is_owner(&self, contact: &ContactId) -> bool {
        if let Some(ref owner) = self.owner_number {
            if owner == contact {
                return true;
            }
        }
        self.allowlist.contains(contact)
    }
}
