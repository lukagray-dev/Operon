//! xAI message normalization and denormalization.
//!
//! xAI uses OpenAI-compatible message structure plus assistant
//! `reasoning_content`, matching DeepSeek semantics.

use operon_context_normalize_reasoning::Provider as ReasoningProvider;
use serde_json::Value;

use crate::error::Result;
use crate::types::ConversationMessage;

use super::openai;

const PROVIDER: &str = "xAI";

/// Normalize an xAI wire message into canonical form.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    openai::normalize_message_with_provider_and_reasoning(
        raw,
        PROVIDER,
        Some("reasoning_content"),
        Some(ReasoningProvider::XAI),
    )
}

/// Denormalize canonical messages into xAI wire message bundle.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    openai::denormalize_messages_with_provider_and_reasoning(
        msgs,
        PROVIDER,
        Some("reasoning_content"),
        Some(ReasoningProvider::XAI),
    )
}
