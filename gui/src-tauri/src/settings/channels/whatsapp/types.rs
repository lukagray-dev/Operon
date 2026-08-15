//! WhatsApp Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// WhatsApp channel state including connection status, credentials, and policy coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppStateDto {
    pub connection_status: String,
    pub owner_number: String,
    pub allowlist: Vec<String>,
    pub workspace_dir: String,
    pub resolved_workspace_placeholder: String,
    pub is_policy_covered: bool,
}

/// Payload to save WhatsApp configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveWhatsAppPayloadDto {
    pub owner_number: String,
    pub allowlist: Vec<String>,
    pub workspace_dir: String,
}
