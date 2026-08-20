use operon_context_normalize_reasoning::Provider as ReasoningProvider;
use serde_json::Value;

use crate::error::Result;
use crate::types::ConversationMessage;

use super::openai;

const PROVIDER: &str = "Groq";

/// Normalize a Groq wire message into canonical form.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    openai::normalize_message_with_provider_and_reasoning(
        raw,
        PROVIDER,
        Some("reasoning_content"),
        Some(ReasoningProvider::Groq),
    )
}

/// Denormalize canonical messages into Groq wire message bundle.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    openai::denormalize_messages_with_provider(msgs, PROVIDER)
}
