//! Canonical conversation-message types.
//!
//! These types are the provider-agnostic representation shared by the Operon
//! context pipeline. Provider-specific wire JSON is normalized into these
//! values on input, and denormalized from these values on output.

use operon_context_normalize_reasoning::ReasoningBlock;
use crate::tools::{ToolCall, ToolResult};
use serde::{Deserialize, Serialize};

pub use crate::stop_reason::StopReason;

/// Canonical role for a conversation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageRole {
    /// End-user input.
    User,
    /// Model output.
    Assistant,
    /// System instruction message.
    System,
    /// Tool result message.
    Tool,
}

/// Canonical image source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImageSource {
    /// Base64-encoded image payload with explicit media type.
    Base64 { media_type: String, data: String },
    /// Remote image URL.
    Url(String),
}

/// Canonical image content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageBlock {
    /// Where the image bytes come from.
    pub source: ImageSource,
}

/// Canonical document source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DocumentSource {
    /// Base64-encoded binary document.
    Base64 { media_type: String, data: String },
    /// Remote document URL.
    Url(String),
    /// Inlined plain text document payload.
    Text(String),
}

/// Canonical document content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentBlock {
    /// Where the document bytes/text come from.
    pub source: DocumentSource,
    /// Optional title metadata for display contexts.
    pub title: Option<String>,
}

/// Canonical conversation content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentBlock {
    /// Plain text content.
    Text(String),
    /// Image content.
    Image(ImageBlock),
    /// Document content.
    Document(DocumentBlock),
    /// Model-emitted tool call.
    ToolCall(ToolCall),
    /// Tool execution result fed back to the model.
    ToolResult(ToolResult),
    /// Reasoning/thinking block from the model.
    Reasoning(ReasoningBlock),
}

/// Canonical conversation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationMessage {
    /// Who authored the message.
    pub role: MessageRole,
    /// Ordered list of content blocks in this message.
    pub content: Vec<ContentBlock>,
    /// Optional stop reason (typically present only on model assistant outputs).
    pub stop_reason: Option<StopReason>,
}

impl ConversationMessage {
    /// Construct a user message with canonical content blocks.
    pub fn user(content: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::User,
            content,
            stop_reason: None,
        }
    }

    /// Construct an assistant message with canonical content blocks.
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content,
            stop_reason: None,
        }
    }

    /// Construct a system message using a single text block.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: vec![ContentBlock::Text(text.into())],
            stop_reason: None,
        }
    }

    /// Attach a stop reason to an existing message using builder style.
    pub fn with_stop(mut self, reason: StopReason) -> Self {
        self.stop_reason = Some(reason);
        self
    }
}
