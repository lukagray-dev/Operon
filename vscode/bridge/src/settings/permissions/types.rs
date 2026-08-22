//! Permissions Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// Summary of allowed directories and default workspace directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedDirectoriesDto {
    pub directories: Vec<String>,
    pub workspace_directory: String,
}

/// Permission item row representing a group or specific tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionItemDto {
    pub key: String,
    pub label: String,
    pub subtitle: String,
    pub mode: String, // "allow", "ask", "deny"
    pub base_mode: String,
    pub is_explicit: bool,
    pub kind: String, // "group" or "tool"
    pub group_key: String,
    pub is_expanded: bool,
    pub has_tools: bool,
}

/// Request payload to update permission mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePermissionRequestDto {
    pub scope: String, // "owner" or "external"
    pub directory: Option<String>,
    pub key: String,
    pub kind: String,
    pub mode: String, // "allow", "ask", "deny"
}
