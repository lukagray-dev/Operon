//! Discord Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// Discord channel state including connection status, credentials, and policy coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordStateDto {
    pub connection_status: String,
    pub bot_token: String,
    pub owner_user_id: String,
    pub allowlist: Vec<String>,
    pub guild_id: String,
    pub workspace_dir: String,
    pub resolved_workspace_placeholder: String,
    pub is_policy_covered: bool,
}

/// Payload to save Discord configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDiscordPayloadDto {
    pub bot_token: String,
    pub owner_user_id: String,
    pub allowlist: Vec<String>,
    pub guild_id: String,
    pub workspace_dir: String,
}

