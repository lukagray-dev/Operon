// workspace.rs — Workspace and session directory management for operon-channels-discord.
//
// Hey friend! This module manages the workspace directory and per-user session JSON storage
// locations for Discord.
//
// Structure:
//   - Shared tool execution workspace: `~/.operon/workspace/` (or custom `DiscordConfig.workspace_dir`).
//   - Isolated session storage: `~/.operon/sessions/discord/<user_id>/<session_id>.json`.
//   - User-specific system instructions (`generate_owner_channel_instructions` vs `generate_external_channel_instructions`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::DiscordError;
use crate::types::UserId;

/// Manages workspace directory provisioning and session file path resolution for Discord.
#[derive(Debug, Clone)]
pub struct DiscordWorkspaceManager {
    shared_workspace_root: PathBuf,
    base_sessions_dir: PathBuf,
}

impl DiscordWorkspaceManager {
    /// Creates a new `DiscordWorkspaceManager` with default paths:
    /// - Shared workspace: `~/.operon/workspace/`
    /// - Base sessions: `~/.operon/sessions/discord/`
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let shared_workspace_root = home.join(".operon").join("workspace");
        let base_sessions_dir = home
            .join(".operon")
            .join("sessions")
            .join("discord");

        Self {
            shared_workspace_root,
            base_sessions_dir,
        }
    }

    /// Creates a `DiscordWorkspaceManager` with explicit workspace and session paths.
    pub fn with_paths(shared_workspace_root: PathBuf, base_sessions_dir: PathBuf) -> Self {
        Self {
            shared_workspace_root,
            base_sessions_dir,
        }
    }

    /// Returns the resolved path to the shared workspace root directory.
    pub fn shared_workspace_root(&self) -> &Path {
        &self.shared_workspace_root
    }

    /// Returns the resolved path to the base sessions directory.
    pub fn base_sessions_dir(&self) -> &Path {
        &self.base_sessions_dir
    }

    /// Returns the path to the user's session directory (`~/.operon/sessions/discord/<user_id>/`).
    pub fn session_dir_for(&self, user_id: &UserId) -> PathBuf {
        self.base_sessions_dir.join(user_id.as_str())
    }

    /// Returns the path to the user's specific session JSON file (`~/.operon/sessions/discord/<user_id>/<session_id>.json`).
    pub fn session_file_path_for(&self, user_id: &UserId, session_id: &str) -> PathBuf {
        self.session_dir_for(user_id)
            .join(format!("{}.json", session_id))
    }

    /// Provisions the shared workspace directory and ensures the user's session folder exists.
    pub fn provision_workspace(
        &self,
        user_id: &UserId,
        _is_owner: bool,
    ) -> Result<PathBuf, DiscordError> {
        if !self.shared_workspace_root.exists() {
            fs::create_dir_all(&self.shared_workspace_root)?;
        }

        let user_session_dir = self.session_dir_for(user_id);
        if !user_session_dir.exists() {
            fs::create_dir_all(&user_session_dir)?;
        }

        Ok(self.shared_workspace_root.clone())
    }
}

impl Default for DiscordWorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generates system instructions for an Owner user interacting via Discord.
pub fn generate_owner_channel_instructions(user_id: &UserId) -> String {
    format!(
        "You are communicating with your Owner via Discord (User ID: {}).\n\
         You have full authorization to assist them with coding, research, system management, and file operations.\n\
         Keep your responses clean, helpful, concise, and structured. If executing tools, progress will be streamed directly.",
        user_id
    )
}

/// Generates system instructions for an External (untrusted/guest) user interacting via Discord.
pub fn generate_external_channel_instructions(user_id: &UserId) -> String {
    format!(
        "You are communicating with an External User via Discord (User ID: {}).\n\
         Your tool access is restricted by policy. You cannot modify protected directories or execute privileged commands without Owner approval.\n\
         Be polite, concise, and helpful within your permitted capabilities.",
        user_id
    )
}

