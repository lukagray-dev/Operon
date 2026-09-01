//! Slack Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// Slack channel state including connection status, credentials, and policy coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackStateDto {
    pub connection_status: String,
    pub bot_token: String,
    pub app_token: String,
    pub owner_user_id: String,
    pub allowlist: Vec<String>,
    pub workspace_dir: String,
    pub resolved_workspace_placeholder: String,
    pub is_policy_covered: bool,
}

/// Payload to save Slack configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSlackPayloadDto {
    pub bot_token: String,
    pub app_token: String,
    pub owner_user_id: String,
    pub allowlist: Vec<String>,
    pub workspace_dir: String,
}

