//! DeepSeek message normalization and denormalization.
//!
//! DeepSeek is OpenAI-compatible for core message structure, with one extra
//! assistant field: `reasoning_content`.

use operon_context_normalize_reasoning::Provider as ReasoningProvider;
use serde_json::Value;

use crate::error::Result;
use crate::types::ConversationMessage;

use super::openai;

const PROVIDER: &str = "DeepSeek";

/// Normalize a DeepSeek wire message into canonical form.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    openai::normalize_message_with_provider_and_reasoning(
        raw,
        PROVIDER,
        Some("reasoning_content"),
        Some(ReasoningProvider::DeepSeek),
    )
}

/// Denormalize canonical messages into DeepSeek wire message bundle.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    openai::denormalize_messages_with_provider_and_reasoning(
        msgs,
        PROVIDER,
        Some("reasoning_content"),
        Some(ReasoningProvider::DeepSeek),
    )
}
