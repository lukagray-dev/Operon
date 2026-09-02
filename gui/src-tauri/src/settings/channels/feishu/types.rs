//! Feishu / Lark Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// Feishu channel state including connection status, credentials, domain, and policy coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuStateDto {
    pub connection_status: String,
    pub app_id: String,
    pub app_secret: String,
    pub domain: String,
    pub owner_user_id: String,
    pub allowlist: Vec<String>,
    pub workspace_dir: String,
    pub resolved_workspace_placeholder: String,
    pub is_policy_covered: bool,
}

/// Payload to save Feishu configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveFeishuPayloadDto {
    pub app_id: String,
    pub app_secret: String,
    pub domain: String,
    pub owner_user_id: String,
    pub allowlist: Vec<String>,
    pub workspace_dir: String,
}

