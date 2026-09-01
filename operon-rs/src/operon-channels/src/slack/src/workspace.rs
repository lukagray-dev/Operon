// workspace.rs — Workspace and session storage isolation for Slack channel.
//
// Hey friend! This module manages disk paths and environment instructions for Slack:
// 1. Session JSON files: `~/.operon/sessions/slack/<user_id>/<session_id>.json`
// 2. Workspace root: Configured directory or `~/.operon/workspace/`
// 3. Generates role-based system prompts for Owner vs External users.

use std::path::PathBuf;
use tracing::info;

use crate::error::SlackError;
use crate::types::UserId;

/// Generates system instruction block for an Owner interacting via Slack.
pub fn generate_owner_channel_instructions(user_id: &UserId) -> String {
    format!(
        "You are interacting with the system Owner via Slack (User ID: {}).\n\
        The owner has full administrative privileges within configured policy boundaries.\n\
        When tools require user approval, a notification will be sent here, and the owner can approve or deny in the Operon Desktop GUI.\n\
        Format your answers cleanly using Slack markdown.",
        user_id
    )
}

/// Generates system instruction block for an External caller interacting via Slack.
pub fn generate_external_channel_instructions(user_id: &UserId) -> String {
    format!(
        "You are interacting with an External User via Slack (User ID: {}).\n\
        External users operate in read-only and restricted mode.\n\
        Tools modifying the filesystem, executing arbitrary bash commands, or changing security configurations are restricted.\n\
        Provide helpful, concise responses formatted cleanly using Slack markdown.",
        user_id
    )
}

/// Manages workspace directories and per-user session storage paths.
pub struct SlackWorkspaceManager {
    base_workspace_dir: PathBuf,
    base_sessions_dir: PathBuf,
}

impl SlackWorkspaceManager {
    /// Creates a manager with default `~/.operon/workspace` and `~/.operon/sessions/slack`.
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let base_workspace_dir = home.join(".operon").join("workspace");
        let base_sessions_dir = home
            .join(".operon")
            .join("sessions")
            .join("slack");
        Self {
            base_workspace_dir,
            base_sessions_dir,
        }
    }

    /// Creates a manager with custom paths.
    pub fn with_paths(base_workspace_dir: PathBuf, base_sessions_dir: PathBuf) -> Self {
        Self {
            base_workspace_dir,
            base_sessions_dir,
        }
    }

    /// Returns the session JSON file path for a user: `~/.operon/sessions/slack/<user_id>/<session_id>.json`.
    pub fn session_file_path_for(&self, user_id: &UserId, session_id: &str) -> PathBuf {
        self.base_sessions_dir
            .join(user_id.as_str())
            .join(format!("{}.json", session_id))
    }

    /// Ensures session and workspace directories exist on disk.
    pub fn provision_workspace(
        &self,
        user_id: &UserId,
        _is_owner: bool,
    ) -> Result<PathBuf, SlackError> {
        let ws = &self.base_workspace_dir;
        if !ws.exists() {
            std::fs::create_dir_all(ws)?;
            info!(path = %ws.display(), "Provisioned Slack shared workspace directory");
        }

        let user_sessions_dir = self.base_sessions_dir.join(user_id.as_str());
        if !user_sessions_dir.exists() {
            std::fs::create_dir_all(&user_sessions_dir)?;
            info!(path = %user_sessions_dir.display(), "Provisioned Slack user sessions directory");
        }

        Ok(ws.clone())
    }
}

