//! Channels Root Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// Summary of a messaging channel for the channels list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCardDto {
    pub id: String,
    pub label: String,
    pub status: String,
    pub is_active: bool,
    pub description: String,
}
