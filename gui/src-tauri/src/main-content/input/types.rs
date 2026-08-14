//! Data Transfer Objects for the Main Content Input Panel.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAttachmentDto {
    pub path: String,
    pub file_name: String,
    pub is_image: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOptionDto {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub context_window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextUsageDto {
    pub tokens_used: usize,
    pub tokens_total: usize,
    pub percentage: f32,
    pub formatted: String,
}
