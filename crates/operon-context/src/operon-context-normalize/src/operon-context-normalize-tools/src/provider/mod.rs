//! Provider enum and the [`FromWire`] / [`ToWire`] trait definitions.
//!
//! This module is the **dispatch hub** of the crate. It defines:
//! - [`Provider`] — an enum identifying which LLM provider's wire format to use.
//! - [`FromWire`] — a trait for converting raw provider JSON into a canonical type.
//! - [`ToWire`] — a trait for converting a canonical type back into provider JSON.
//!
//! Each canonical type ([`ToolCall`], [`ToolDefinition`], [`ToolResult`]) implements
//! one or both traits. The implementations match on the [`Provider`] variant and
//! forward to the relevant provider-specific module.
//!
//! # Adding a new provider
//! 1. Add a variant to [`Provider`].
//! 2. Create `src/provider/<name>.rs` implementing the three public functions:
//!    `from_wire_tool_call`, `to_wire_tool_definition`, `to_wire_tool_result`.
//! 3. Declare the module here and add match arms to the three trait implementations.

pub mod anthropic;
pub mod cohere;
pub mod deepseek;
pub mod gemini;
pub mod groq;
pub mod mistral;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod xai;

use serde_json::Value;

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolDefinition, ToolResult};

// ─────────────────────────────────────────────────────────────────────────────
// Provider enum
// ─────────────────────────────────────────────────────────────────────────────

/// Identifies which LLM provider's wire format should be used for normalization
/// or denormalization.
///
/// Pass a reference to this enum to [`normalize`](crate::normalize),
/// [`denormalize_definition`](crate::denormalize_definition), and
/// [`denormalize_result`](crate::denormalize_result).
///
/// # Example
/// ```
/// use operon_context_normalize_tools::Provider;
///
/// let provider = Provider::Anthropic;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Provider {
    /// Anthropic Claude models (claude-3-*, claude-opus-*, etc.)
    Anthropic,
    /// OpenAI GPT models (gpt-4o, gpt-4-turbo, gpt-3.5-turbo, etc.)
    OpenAI,
    /// Google Gemini models (gemini-1.5-pro, gemini-flash, etc.)
    Gemini,
    /// Ollama local model server (OpenAI-compatible wire format)
    Ollama,
    /// DeepSeek models (OpenAI-compatible wire format)
    DeepSeek,
    /// OpenRouter gateway (auto-detects underlying provider shape)
    OpenRouter,
    /// Groq inference API (OpenAI-compatible wire format)
    Groq,
    /// Mistral AI models (OpenAI-compatible wire format)
    Mistral,
    /// xAI Grok models (OpenAI-compatible wire format)
    XAI,
    /// Cohere Command models (distinct wire format with parameter_definitions)
    Cohere,
}

// ─────────────────────────────────────────────────────────────────────────────
// Traits
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a raw provider wire JSON value into a canonical type.
///
/// Implemented by [`ToolCall`] — call it via [`normalize`](crate::normalize) rather
/// than directly, unless you need lower-level access.
///
/// # Example
/// ```
/// use operon_context_normalize_tools::{Provider, ToolCall};
/// use operon_context_normalize_tools::provider::FromWire;
/// use serde_json::json;
///
/// let raw = json!({
///     "id": "call_abc",
///     "type": "function",
///     "function": { "name": "my_tool", "arguments": "{\"x\":1}" }
/// });
/// let call = ToolCall::from_wire(raw, &Provider::OpenAI).unwrap();
/// assert_eq!(call.name, "my_tool");
/// ```
pub trait FromWire: Sized {
    /// Deserialize `raw` according to `provider`'s wire format.
    fn from_wire(raw: Value, provider: &Provider) -> Result<Self, ToolNormalizeError>;
}

/// Convert a canonical type into a provider-specific wire JSON value.
///
/// Implemented by [`ToolDefinition`] and [`ToolResult`]. Use
/// [`denormalize_definition`](crate::denormalize_definition) and
/// [`denormalize_result`](crate::denormalize_result) for the public API.
pub trait ToWire {
    /// Serialize `self` according to `provider`'s wire format.
    fn to_wire(&self, provider: &Provider) -> Result<Value, ToolNormalizeError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Trait implementations — dispatch to provider modules
// ─────────────────────────────────────────────────────────────────────────────

impl FromWire for ToolCall {
    /// Dispatch to the provider-specific `from_wire_tool_call` function.
    fn from_wire(raw: Value, provider: &Provider) -> Result<Self, ToolNormalizeError> {
        match provider {
            Provider::Anthropic  => anthropic::from_wire_tool_call(raw),
            Provider::OpenAI     => openai::from_wire_tool_call(raw),
            Provider::Gemini     => gemini::from_wire_tool_call(raw),
            Provider::Ollama     => ollama::from_wire_tool_call(raw),
            Provider::DeepSeek   => deepseek::from_wire_tool_call(raw),
            Provider::OpenRouter => openrouter::from_wire_tool_call(raw),
            Provider::Groq       => groq::from_wire_tool_call(raw),
            Provider::Mistral    => mistral::from_wire_tool_call(raw),
            Provider::XAI        => xai::from_wire_tool_call(raw),
            Provider::Cohere     => cohere::from_wire_tool_call(raw),
        }
    }
}

impl ToWire for ToolDefinition {
    /// Dispatch to the provider-specific `to_wire_tool_definition` function.
    fn to_wire(&self, provider: &Provider) -> Result<Value, ToolNormalizeError> {
        match provider {
            Provider::Anthropic  => anthropic::to_wire_tool_definition(self),
            Provider::OpenAI     => openai::to_wire_tool_definition(self),
            Provider::Gemini     => gemini::to_wire_tool_definition(self),
            Provider::Ollama     => ollama::to_wire_tool_definition(self),
            Provider::DeepSeek   => deepseek::to_wire_tool_definition(self),
            Provider::OpenRouter => openrouter::to_wire_tool_definition(self),
            Provider::Groq       => groq::to_wire_tool_definition(self),
            Provider::Mistral    => mistral::to_wire_tool_definition(self),
            Provider::XAI        => xai::to_wire_tool_definition(self),
            Provider::Cohere     => cohere::to_wire_tool_definition(self),
        }
    }
}

impl ToWire for ToolResult {
    /// Dispatch to the provider-specific `to_wire_tool_result` function.
    fn to_wire(&self, provider: &Provider) -> Result<Value, ToolNormalizeError> {
        match provider {
            Provider::Anthropic  => anthropic::to_wire_tool_result(self),
            Provider::OpenAI     => openai::to_wire_tool_result(self),
            Provider::Gemini     => gemini::to_wire_tool_result(self),
            Provider::Ollama     => ollama::to_wire_tool_result(self),
            Provider::DeepSeek   => deepseek::to_wire_tool_result(self),
            Provider::OpenRouter => openrouter::to_wire_tool_result(self),
            Provider::Groq       => groq::to_wire_tool_result(self),
            Provider::Mistral    => mistral::to_wire_tool_result(self),
            Provider::XAI        => xai::to_wire_tool_result(self),
            Provider::Cohere     => cohere::to_wire_tool_result(self),
        }
    }
}
