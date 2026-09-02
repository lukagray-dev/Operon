// workspace.rs — Workspace and session storage management for Feishu / Lark.
//
// Hey friend! This module handles workspace provisioning and session store paths for Feishu:
// 1. Session store location: `~/.operon/sessions/feishu/<user_id>/<session_id>.json`.
// 2. Shared workspace directory resolution (`~/.operon/workspace/`).
// 3. Role-specific system instructions for Owner vs External callers.

use std::path::PathBuf;
use tracing::info;

use crate::error::FeishuError;
use crate::types::UserId;

/// Manages workspace roots and session file paths for Feishu users.
#[derive(Debug, Clone)]
pub struct FeishuWorkspaceManager {
    workspace_root: PathBuf,
    sessions_base_dir: PathBuf,
}

impl FeishuWorkspaceManager {
    /// Creates a new `FeishuWorkspaceManager` with explicit paths.
    pub fn with_paths(workspace_root: PathBuf, sessions_base_dir: PathBuf) -> Self {
        Self {
            workspace_root,
            sessions_base_dir,
        }
    }

    /// Computes the session store file path for a user session.
    pub fn session_file_path_for(&self, user_id: &UserId, session_id: &str) -> PathBuf {
        self.sessions_base_dir
            .join(user_id.as_str())
            .join(format!("{}.json", session_id))
    }

    /// Provisions workspace root and user session directory on disk.
    pub fn provision_workspace(
        &self,
        user_id: &UserId,
        _is_owner: bool,
    ) -> Result<PathBuf, FeishuError> {
        if !self.workspace_root.exists() {
            std::fs::create_dir_all(&self.workspace_root)?;
            info!(
                "Created Feishu workspace directory at {}",
                self.workspace_root.display()
            );
        }

        let user_session_dir = self.sessions_base_dir.join(user_id.as_str());
        if !user_session_dir.exists() {
            std::fs::create_dir_all(&user_session_dir)?;
            info!(
                "Created Feishu user session directory at {}",
                user_session_dir.display()
            );
        }

        Ok(self.workspace_root.clone())
    }
}

/// Generates system instructions for the authenticated Owner over Feishu.
pub fn generate_owner_channel_instructions(user_id: &UserId) -> String {
    format!(
        "You are Operon, an autonomous AI system assistant interacting with the Owner via Feishu / Lark (User ID: {}).\n\
        The user has full Owner authority. Respond concisely and clearly in Markdown. Tool calls will execute according to policy rules.",
        user_id
    )
}

/// Generates system instructions for an External (non-owner) caller over Feishu.
pub fn generate_external_channel_instructions(user_id: &UserId) -> String {
    format!(
        "You are Operon, an autonomous AI assistant interacting with an External User via Feishu / Lark (User ID: {}).\n\
        You have restricted permissions. Do not reveal private system credentials or execute destructive operations without permission.",
        user_id
    )
}

