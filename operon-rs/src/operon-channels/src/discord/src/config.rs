// config.rs — Configuration management for operon-channels-discord.
//
// Hey friend! This module handles settings for the Discord channel, including the bot token,
// owner user ID, allowed contact IDs, optional guild restriction, and workspace paths.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::UserId;

/// Configuration parameters for Discord channel integration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// Whether the Discord channel is enabled.
    pub enabled: bool,
    /// Discord bot token obtained from the Discord Developer Portal.
    pub bot_token: Option<String>,
    /// Discord user snowflake ID of the agent owner (granted `Role::Owner` privileges).
    pub owner_user_id: Option<UserId>,
    /// List of allowed Discord user snowflake IDs granted `Role::Owner` privileges.
    pub allowlist: Vec<UserId>,
    /// Optional guild (server) ID to restrict bot operations to a specific server.
    pub guild_id: Option<String>,
    /// Custom path for shared workspace root directory for Discord session tool calls.
    /// Defaults to global agent workspace (`~/.operon/workspace/`).
    pub workspace_dir: Option<PathBuf>,
}

impl DiscordConfig {
    /// Returns the base Discord REST API URL (v10).
    pub fn base_api_url(&self) -> &'static str {
        "https://discord.com/api/v10"
    }

    /// Returns the resolved workspace directory path for Discord session turns.
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
    /// If Discord is enabled and no policy entry covers `resolved_workspace_dir()`, logs a clear warning
    /// advising the user to configure policy coverage so tool calls won't silently deny.
    pub fn check_policy_coverage(&self, policy: &operon_config::PolicyConfig) -> bool {
        let resolved_ws = self.resolved_workspace_dir();
        let canonical_ws = std::fs::canonicalize(&resolved_ws)
            .map(clean_verbatim_path)
            .unwrap_or_else(|_| resolved_ws.clone());
        let covered = policy.any_directory_covers(&canonical_ws);
        if self.enabled && !covered {
            tracing::warn!(
                workspace_dir = %resolved_ws.display(),
                "Discord channel is enabled, but no DirectoryPolicy entry in PolicyConfig covers workspace directory '{}'. All Discord tool calls will silently Deny. Please add a DirectoryPolicy for this path in your policy configuration.",
                resolved_ws.display()
            );
        }
        covered
    }

    /// Checks if a given user ID is considered an Owner (main owner or allowlisted).
    pub fn is_owner(&self, user_id: &UserId) -> bool {
        if let Some(ref owner) = self.owner_user_id {
            if owner == user_id {
                return true;
            }
        }
        self.allowlist.contains(user_id)
    }
}

/// Strips the Windows verbatim/extended-length prefix (`\\?\` and `\\?\UNC\`) from a path.
pub fn clean_verbatim_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let path_str = path.to_string_lossy();
        if let Some(stripped) = path_str.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{}", stripped))
        } else if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            path
        }
    }
    #[cfg(not(windows))]
    {
        path
    }
}

