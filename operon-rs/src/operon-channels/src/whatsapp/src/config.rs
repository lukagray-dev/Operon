// config.rs — Configuration management for operon-channels-whatsapp.
//
// Hey friend! This module handles settings for the WhatsApp channel, including the main owner
// phone number, allowed contact numbers, auto-reply behavior, and authentication directories.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::ContactId;

/// Configuration parameters for WhatsApp channel integration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Custom path for shared workspace root directory for WhatsApp session tool calls.
    /// Defaults to global agent workspace (`~/.operon/workspace/`).
    pub workspace_dir: Option<PathBuf>,
}

impl WhatsAppConfig {
    /// Returns the resolved directory path for auth credential storage.
    pub fn resolved_auth_dir(&self) -> PathBuf {
        if let Some(ref path) = self.auth_dir {
            path.clone()
        } else if let Some(home) = dirs::home_dir() {
            home.join(".operon")
                .join("channels")
                .join("whatsapp")
                .join("auth")
        } else {
            PathBuf::from(".operon/channels/whatsapp/auth")
        }
    }

    /// Returns the resolved workspace directory path for WhatsApp session turns.
    ///
    /// If `workspace_dir` is explicitly set, it is returned. Otherwise, it falls back to the
    /// global default workspace root used by GUI/TUI sessions (`~/.operon/workspace/`).
    pub fn resolved_workspace_dir(&self) -> PathBuf {
        if let Some(ref path) = self.workspace_dir {
            path.clone()
        } else if let Ok(paths) = operon_config::OperonPaths::resolve() {
            paths.workspace_dir
        } else if let Some(home) = dirs::home_dir() {
            home.join(".operon").join("workspace")
        } else {
            PathBuf::from(".operon/workspace")
        }
    }

    /// Checks if the resolved workspace directory is covered by any `DirectoryPolicy` entry in `PolicyConfig`.
    ///
    /// If WhatsApp is enabled and no policy entry covers `resolved_workspace_dir()`, logs a clear warning
    /// advising the user to configure policy coverage so tool calls won't silently deny.
    pub fn check_policy_coverage(&self, policy: &operon_config::PolicyConfig) -> bool {
        let resolved_ws = self.resolved_workspace_dir();
        let canonical_ws =
            std::fs::canonicalize(&resolved_ws).unwrap_or_else(|_| resolved_ws.clone());
        let covered = policy.any_directory_covers(&canonical_ws);
        if self.enabled && !covered {
            tracing::warn!(
                workspace_dir = %resolved_ws.display(),
                "WhatsApp channel is enabled, but no DirectoryPolicy entry in PolicyConfig covers workspace directory '{}'. All WhatsApp tool calls will silently Deny. Please add a DirectoryPolicy for this path in your policy configuration.",
                resolved_ws.display()
            );
        }
        covered
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
