//! Provider enum and dispatch traits for message normalization.
//!
//! This module defines:
//! - [`Provider`]: which wire format to use.
//! - [`FromWireMessage`]: wire JSON -> canonical message conversion trait.
//! - [`ToWireMessages`]: canonical messages -> wire JSON bundle conversion trait.

pub mod anthropic;
pub mod cohere;
pub mod deepseek;
pub mod gemini;
pub mod groq;
pub mod mistral;
pub mod nvidia_nim;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod xai;

use serde_json::Value;

use crate::error::MessageNormalizeError;
use crate::types::ConversationMessage;

// ─────────────────────────────────────────────────────────────────────────────
// Provider re-export
// ─────────────────────────────────────────────────────────────────────────────

// `Provider` is defined in `operon-providers` — the single authoritative source.
// DO NOT redefine here. Add variants in operon-providers/src/provider.rs only.
pub use operon_providers::Provider;

/// Convert one provider wire message payload into canonical type.
pub trait FromWireMessage: Sized {
    /// Parse `raw` according to `provider` wire format.
    fn from_wire(raw: Value, provider: &Provider) -> Result<Self, MessageNormalizeError>;
}

/// Convert canonical messages into provider wire-format JSON bundle.
pub trait ToWireMessages {
    /// Serialize as `{ "messages": [...], "system": <string-or-null> }`.
    fn to_wire(&self, provider: &Provider) -> Result<Value, MessageNormalizeError>;
}

impl FromWireMessage for ConversationMessage {
    /// Dispatch conversion to provider-specific module.
    fn from_wire(raw: Value, provider: &Provider) -> Result<Self, MessageNormalizeError> {
        match provider {
            Provider::Anthropic => anthropic::normalize_message(raw),
            Provider::OpenAI => openai::normalize_message(raw),
            Provider::Gemini => gemini::normalize_message(raw),
            Provider::Ollama => ollama::normalize_message(raw),
            Provider::DeepSeek => deepseek::normalize_message(raw),
            Provider::OpenRouter => openrouter::normalize_message(raw),
            Provider::Groq => groq::normalize_message(raw),
            Provider::Mistral => mistral::normalize_message(raw),
            Provider::XAI => xai::normalize_message(raw),
            Provider::NvidiaNim => nvidia_nim::normalize_message(raw),
            Provider::Cohere => cohere::normalize_message(raw),
        }
    }
}

impl ToWireMessages for Vec<ConversationMessage> {
    /// Dispatch conversion to provider-specific module.
    fn to_wire(&self, provider: &Provider) -> Result<Value, MessageNormalizeError> {
        match provider {
            Provider::Anthropic => anthropic::denormalize_messages(self),
            Provider::OpenAI => openai::denormalize_messages(self),
            Provider::Gemini => gemini::denormalize_messages(self),
            Provider::Ollama => ollama::denormalize_messages(self),
            Provider::DeepSeek => deepseek::denormalize_messages(self),
            Provider::OpenRouter => openrouter::denormalize_messages(self),
            Provider::Groq => groq::denormalize_messages(self),
            Provider::Mistral => mistral::denormalize_messages(self),
            Provider::XAI => xai::denormalize_messages(self),
            Provider::NvidiaNim => nvidia_nim::denormalize_messages(self),
            Provider::Cohere => cohere::denormalize_messages(self),
        }
    }
}
