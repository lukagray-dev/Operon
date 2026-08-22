//! Telegram Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// Telegram channel state including connection status, credentials, and policy coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramStateDto {
    pub connection_status: String,
    pub bot_token: String,
    pub owner_chat_id: String,
    pub allowlist: Vec<String>,
    pub workspace_dir: String,
    pub resolved_workspace_placeholder: String,
    pub is_policy_covered: bool,
}

/// Payload to save Telegram configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTelegramPayloadDto {
    pub bot_token: String,
    pub owner_chat_id: String,
    pub allowlist: Vec<String>,
    pub workspace_dir: String,
}
