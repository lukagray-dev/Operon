//! `operon-context-normalize-reasoning` Standardizes reasoning/thinking content across AI providers.
//!
//! # Purpose
//! This crate is designed to normalize LLM reasoning and thinking content from provider-specific
//! formats (like Anthropic's thinking blocks, OpenAI's reasoning_summary arrays, DeepSeek's
//! reasoning_content strings, and Gemini's thought parts) into a single, unified canonical
//! format represented by [`ReasoningBlock`] and [`ReasoningSignature`].
//!
//! It also supports denormalizing canonical [`ReasoningBlock`]s back into the provider-specific wire JSON.
//!
//! # Architecture
//! - [`types.rs`]: Defines canonical representations [`ReasoningBlock`] and [`ReasoningSignature`].
//! - [`error.rs`]: Defines the error type [`ReasoningNormalizeError`].
//! - [`provider`]: The module containing provider implementations and the [`FromWireReasoning`] and [`ToWireReasoning`] traits.
//! - [`lib.rs`]: The public API entry point providing [`normalize_reasoning`] and [`denormalize_reasoning`].
//!
//! # Examples
//!
//! ## Normalizing reasoning from Anthropic:
//! ```rust
//! use operon_context_normalize_reasoning::{normalize_reasoning, Provider, ReasoningBlock};
//! use serde_json::json;
//!
//! let raw = json!({
//!     "type": "thinking",
//!     "thinking": "Decomposing the request...",
//!     "signature": "EqoBCkgIAR..."
//! });
//!
//! let blocks = normalize_reasoning(raw, &Provider::Anthropic).unwrap();
//! assert_eq!(blocks[0].thinking, "Decomposing the request...");
//! assert_eq!(blocks[0].signature.as_ref().unwrap().0, "EqoBCkgIAR...");
//! ```
//!
//! ## Denormalizing reasoning back to DeepSeek:
//! ```rust
//! use operon_context_normalize_reasoning::{denormalize_reasoning, Provider, ReasoningBlock};
//!
//! let blocks = vec![ReasoningBlock::new("Decomposing the request...")];
//! let wire = denormalize_reasoning(&blocks, &Provider::DeepSeek).unwrap();
//! assert_eq!(wire, serde_json::json!("Decomposing the request..."));
//! ```

pub mod error;
pub mod provider;
pub mod types;

// Re-export public types at the root level of the crate for user convenience.
pub use error::ReasoningNormalizeError;
pub use provider::{FromWireReasoning, Provider, ToWireReasoning};
pub use types::{ReasoningBlock, ReasoningSignature};

use serde_json::Value;

/// Standardize a provider's raw reasoning/thinking wire payload into canonical [`ReasoningBlock`]s.
///
/// This is the primary entry point for parsing and normalizing reasoning payload. The structure of
/// `raw` depends on the provider (e.g., a single content block object for Anthropic, a string for DeepSeek,
/// an array for OpenAI).
///
/// # Arguments
/// * `raw` - The raw JSON payload containing the reasoning content.
/// * `provider` - The LLM provider whose format we should parse.
///
/// # Returns
/// * `Ok(Vec<ReasoningBlock>)` containing the normalized blocks.
/// * `Err(ReasoningNormalizeError)` if the payload is missing required fields or is malformed.
pub fn normalize_reasoning(
    raw: Value,
    provider: &Provider,
) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // We delegate the call to the FromWireReasoning trait implementation on ReasoningBlock.
    ReasoningBlock::from_wire(raw, provider)
}

/// Convert canonical [`ReasoningBlock`]s back into a provider-specific reasoning wire JSON value.
///
/// This is the primary entry point for serializing reasoning blocks back into wire format for sending
/// to the model's API.
///
/// # Arguments
/// * `blocks` - A slice of canonical reasoning blocks.
/// * `provider` - The LLM provider format we want to generate.
///
/// # Returns
/// * `Ok(Value)` representing the provider-specific JSON payload.
/// * `Err(ReasoningNormalizeError)` if serialization fails.
pub fn denormalize_reasoning(
    blocks: &[ReasoningBlock],
    provider: &Provider,
) -> Result<Value, ReasoningNormalizeError> {
    // Since ToWireReasoning is implemented on Vec<ReasoningBlock>, we clone the slice into a Vec
    // and invoke the to_wire method.
    blocks.to_vec().to_wire(provider)
}
