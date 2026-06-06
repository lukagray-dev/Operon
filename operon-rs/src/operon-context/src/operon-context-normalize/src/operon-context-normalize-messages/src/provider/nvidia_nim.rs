//! NVIDIA NIM message normalization and denormalization.
//!
//! NIM is fully OpenAI-compatible. All logic delegates to [`super::openai`].

use operon_context_normalize_reasoning::Provider as ReasoningProvider;
use serde_json::Value;

use crate::error::Result;
use crate::types::ConversationMessage;

use super::openai;

/// The provider identifier string used in error reporting.
const PROVIDER: &str = "NVIDIA NIM";

/// Normalize a NIM wire message into canonical form.
///
/// Since NVIDIA NIM uses the standard OpenAI chat completions API format,
/// we delegate the normalization process directly to the OpenAI implementation.
/// The `PROVIDER` string is passed along to ensure that any validation errors
/// reference "NVIDIA NIM" instead of OpenAI.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    openai::normalize_message_with_provider_and_reasoning(
        raw,
        PROVIDER,
        Some("reasoning_content"),
        Some(ReasoningProvider::NvidiaNim),
    )
}

/// Denormalize canonical messages into NIM wire message bundle.
///
/// Converts our internal `ConversationMessage` format into the OpenAI-compatible
/// wire format expected by the NVIDIA NIM endpoint.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    openai::denormalize_messages_with_provider_and_reasoning(
        msgs,
        PROVIDER,
        Some("reasoning_content"),
        Some(ReasoningProvider::NvidiaNim),
    )
}
