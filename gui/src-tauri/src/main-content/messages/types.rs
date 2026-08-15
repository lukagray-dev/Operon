//! Data Transfer Objects for Conversation Messages.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageBlockDto {
    WorkGroup {
        data: crate::main_content::work_group::WorkGroupDto,
    },
    Text {
        text: String,
    },
}

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
    pub work_group: Option<crate::main_content::work_group::WorkGroupDto>,
    pub blocks: Option<Vec<MessageBlockDto>>,
}
