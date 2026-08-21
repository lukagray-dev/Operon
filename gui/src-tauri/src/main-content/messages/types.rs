//! Data Transfer Objects for Conversation Messages.
//!
//! Hey friend! These types are serialized across the Tauri IPC boundary to the frontend webview.
//! They represent structured messages, streaming blocks, and interactive ask question prompts.

use serde::{Deserialize, Serialize};

/// Represents an interactive clarifying question prompt from the `ask` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskQuestionDto {
    /// Unique identifier for this ask prompt matching the tool call id.
    pub id: String,
    /// The question text asked by the model.
    pub question: String,
    /// Pre-defined multiple-choice answer options (normally 3).
    pub options: Vec<String>,
    /// Selected or submitted answer, if answered.
    pub answer: Option<String>,
    /// Whether the user has already answered this question.
    pub is_answered: bool,
}

/// Represents an expandable context compaction event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionDto {
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub summary: String,
    pub is_expanded: bool,
}

/// A block within a consolidated assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageBlockDto {
    /// Progress work group containing thinking steps or tool calls.
    WorkGroup {
        data: crate::main_content::work_group::WorkGroupDto,
    },
    /// Expandable context compaction pill.
    Compaction { data: CompactionDto },
    /// Text response content block.
    Text { text: String },
    /// Interactive or historical ask question prompt.
    Ask { data: AskQuestionDto },
}

/// High-level chat message DTO passed to the GUI frontend.
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
