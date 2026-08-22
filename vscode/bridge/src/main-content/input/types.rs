//! Data Transfer Objects for the Main Content Input Panel.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAttachmentDto {
    pub path: String,
    pub file_name: String,
    pub is_image: bool,
    pub size_bytes: u64,
}

/// Data Transfer Object representing an AI model choice in the GUI input panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOptionDto {
    /// Unique identifier string sent in API requests (e.g., "claude-3-7-sonnet-20250219", "gpt-4o").
    pub id: String,
    /// Human-friendly display name of the model.
    pub name: String,
    /// Whether this model is currently the active model selected in settings/session.
    pub is_active: bool,
    /// Maximum context window token capacity for the model.
    pub context_window: usize,
    /// List of reasoning levels supported by this model as fetched from the provider API
    /// (e.g. `["Low", "Medium", "High", "Max"]`). If empty, the model does not support reasoning.
    #[serde(default)]
    pub reasoning_levels: Vec<String>,
    /// The currently selected reasoning effort for this model if active (e.g. `Some("High")`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextUsageDto {
    pub tokens_used: usize,
    pub tokens_total: usize,
    pub percentage: f32,
    pub formatted: String,
}
