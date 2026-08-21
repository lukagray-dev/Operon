// config.rs — Configuration management for operon-channels-telegram.
//
// Hey friend! This module handles settings for the Telegram channel, including the bot token,
// main owner chat ID, allowed chat IDs, poll interval, and workspace directory resolution.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::TelegramError;
use crate::types::ChatId;

/// Configuration parameters for Telegram channel integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Is the Telegram channel enabled.
    pub enabled: bool,
    /// Telegram bot token obtained from @BotFather.
    pub bot_token: Option<String>,
    /// Main owner chat ID granted `CallerRole::Owner` privileges.
    pub owner_chat_id: Option<ChatId>,
    /// List of allowed chat IDs granted `CallerRole::Owner` privileges.
    pub allowlist: Vec<ChatId>,
    /// Custom path for shared workspace root directory for Telegram session tool calls.
    /// Defaults to global agent workspace (`~/.operon/workspace/`).
    pub workspace_dir: Option<PathBuf>,
    /// Long polling timeout interval in seconds (defaults to 30s).
    pub poll_interval_secs: Option<u64>,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_token: None,
            owner_chat_id: None,
            allowlist: Vec::new(),
            workspace_dir: None,
            poll_interval_secs: Some(30),
        }
    }
}

impl TelegramConfig {
    /// Constructs the base API URL for the Telegram Bot API (`https://api.telegram.org/bot<token>`).
    ///
    /// # Errors
    /// Returns `TelegramError::MissingBotToken` if `bot_token` is `None` or empty.
    pub fn base_api_url(&self) -> Result<String, TelegramError> {
        match self.bot_token {
            Some(ref token) if !token.trim().is_empty() => {
                Ok(format!("https://api.telegram.org/bot{}", token.trim()))
            }
            _ => Err(TelegramError::MissingBotToken),
        }
    }

    /// Returns the resolved workspace directory path for Telegram session turns.
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
    /// If Telegram is enabled and no policy entry covers `resolved_workspace_dir()`, logs a clear warning
    /// advising the user to configure policy coverage so tool calls won't silently deny.
    pub fn check_policy_coverage(&self, policy: &operon_config::PolicyConfig) -> bool {
        let resolved_ws = self.resolved_workspace_dir();
        let canonical_ws =
            std::fs::canonicalize(&resolved_ws).unwrap_or_else(|_| resolved_ws.clone());
        let covered = policy.any_directory_covers(&canonical_ws);
        if self.enabled && !covered {
            tracing::warn!(
                workspace_dir = %resolved_ws.display(),
                "Telegram channel is enabled, but no DirectoryPolicy entry in PolicyConfig covers workspace directory '{}'. All Telegram tool calls will silently Deny. Please add a DirectoryPolicy for this path in your policy configuration.",
                resolved_ws.display()
            );
        }
        covered
    }

    /// Checks if a given chat ID is considered an Owner (main owner_chat_id or allowlisted).
    pub fn is_owner(&self, chat_id: &ChatId) -> bool {
        if let Some(ref owner) = self.owner_chat_id {
            if owner == chat_id {
                return true;
            }
        }
        self.allowlist.contains(chat_id)
    }
}
