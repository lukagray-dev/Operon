//! Provider enum and the [`FromWireReasoning`] / [`ToWireReasoning`] trait
//! definitions.
//!
//! This module is the **dispatch hub** of the crate. It defines:
//! - [`Provider`] — identifies which LLM provider's wire format to use.
//! - [`FromWireReasoning`] — converts raw provider JSON into canonical
//!   [`ReasoningBlock`]s.
//! - [`ToWireReasoning`] — converts canonical blocks back into provider JSON.
//!
//! [`FromWireReasoning`] is implemented on [`ReasoningBlock`] (so `from_wire`
//! returns `Vec<ReasoningBlock>`, matching the trait's `Vec<Self>` return type).
//! [`ToWireReasoning`] is implemented on `Vec<ReasoningBlock>` since the whole
//! slice is needed to produce a single wire value (e.g., OpenAI and Gemini
//! serialize arrays natively).
//!
//! # Adding a new provider
//! 1. Add a variant to [`Provider`].
//! 2. Create `src/provider/<name>.rs` with `from_wire_reasoning` and
//!    `to_wire_reasoning` functions.
//! 3. Declare the module here and add match arms to both trait implementations.

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

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

// ─────────────────────────────────────────────────────────────────────────────
// Provider re-export
// ─────────────────────────────────────────────────────────────────────────────

// `Provider` is defined in `operon-providers` — the single authoritative source.
// The comment that previously said "must stay in sync" is now obsolete — there
// is only one enum to maintain.
// DO NOT redefine here. Add variants in operon-providers/src/provider.rs only.
pub use operon_providers::Provider;

// ─────────────────────────────────────────────────────────────────────────────
// Traits
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a raw provider reasoning wire payload into canonical
/// [`ReasoningBlock`]s.
///
/// The shape of `raw` differs per provider — consult each provider module's
/// documentation for exactly what value to pass (e.g., a single content block
/// for Anthropic, the full `reasoning_summary` array for OpenAI).
///
/// Implemented on [`ReasoningBlock`]. Prefer the public
/// [`normalize_reasoning`](crate::normalize_reasoning) function rather than
/// calling this trait directly.
///
/// # Example
/// ```
/// use operon_context_normalize_reasoning::{Provider, ReasoningBlock};
/// use operon_context_normalize_reasoning::provider::FromWireReasoning;
/// use serde_json::json;
///
/// let raw = json!({ "type": "thinking", "thinking": "Step 1: decompose." });
/// let blocks = ReasoningBlock::from_wire(raw, &Provider::Anthropic).unwrap();
/// assert_eq!(blocks[0].thinking, "Step 1: decompose.");
/// ```
pub trait FromWireReasoning: Sized {
    /// Deserialize `raw` according to `provider`'s reasoning wire format.
    ///
    /// Returns a `Vec` because some providers return multiple reasoning blocks
    /// from a single wire value (e.g., OpenAI's `reasoning_summary` array
    /// can contain multiple `summary_text` elements).
    fn from_wire(
        raw: Value,
        provider: &Provider,
    ) -> Result<Vec<Self>, ReasoningNormalizeError>;
}

/// Convert canonical [`ReasoningBlock`]s back to a provider reasoning wire
/// JSON value.
///
/// The shape of the returned `Value` differs per provider (a JSON array for
/// Anthropic and Gemini, a plain string for DeepSeek/xAI/Ollama, etc.).
///
/// Implemented on `Vec<ReasoningBlock>`. Prefer the public
/// [`denormalize_reasoning`](crate::denormalize_reasoning) function rather
/// than calling this trait directly.
///
/// # Example
/// ```
/// use operon_context_normalize_reasoning::{Provider, ReasoningBlock};
/// use operon_context_normalize_reasoning::provider::ToWireReasoning;
///
/// let blocks = vec![ReasoningBlock::new("My analysis here.")];
/// let wire = blocks.to_wire(&Provider::DeepSeek).unwrap();
/// assert_eq!(wire, serde_json::json!("My analysis here."));
/// ```
pub trait ToWireReasoning {
    /// Serialize `self` according to `provider`'s reasoning wire format.
    fn to_wire(&self, provider: &Provider) -> Result<Value, ReasoningNormalizeError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Trait implementations — dispatch to provider modules
// ─────────────────────────────────────────────────────────────────────────────

impl FromWireReasoning for ReasoningBlock {
    /// Match on the provider and forward to the appropriate provider module's
    /// `from_wire_reasoning` function.
    fn from_wire(raw: Value, provider: &Provider) -> Result<Vec<Self>, ReasoningNormalizeError> {
        match provider {
            Provider::Anthropic  => anthropic::from_wire_reasoning(raw),
            Provider::OpenAI     => openai::from_wire_reasoning(raw),
            Provider::Gemini     => gemini::from_wire_reasoning(raw),
            Provider::Ollama     => ollama::from_wire_reasoning(raw),
            Provider::DeepSeek   => deepseek::from_wire_reasoning(raw),
            Provider::OpenRouter => openrouter::from_wire_reasoning(raw),
            Provider::Groq       => groq::from_wire_reasoning(raw),
            Provider::Mistral    => mistral::from_wire_reasoning(raw),
            Provider::XAI        => xai::from_wire_reasoning(raw),
            Provider::Cohere     => cohere::from_wire_reasoning(raw),
        }
    }
}

impl ToWireReasoning for Vec<ReasoningBlock> {
    /// Match on the provider and forward to the appropriate provider module's
    /// `to_wire_reasoning` function.
    fn to_wire(&self, provider: &Provider) -> Result<Value, ReasoningNormalizeError> {
        match provider {
            Provider::Anthropic  => anthropic::to_wire_reasoning(self),
            Provider::OpenAI     => openai::to_wire_reasoning(self),
            Provider::Gemini     => gemini::to_wire_reasoning(self),
            Provider::Ollama     => ollama::to_wire_reasoning(self),
            Provider::DeepSeek   => deepseek::to_wire_reasoning(self),
            Provider::OpenRouter => openrouter::to_wire_reasoning(self),
            Provider::Groq       => groq::to_wire_reasoning(self),
            Provider::Mistral    => mistral::to_wire_reasoning(self),
            Provider::XAI        => xai::to_wire_reasoning(self),
            Provider::Cohere     => cohere::to_wire_reasoning(self),
        }
    }
}
