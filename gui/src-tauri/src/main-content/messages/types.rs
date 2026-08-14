//! Data Transfer Objects for Conversation Messages.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageDto {
    pub id: String,
    pub role: String,
    pub text: String,
    pub timestamp: String,
    pub created_at: i64,
    pub turn_index: usize,
    pub is_liked: bool,
    pub is_disliked: bool,
}
