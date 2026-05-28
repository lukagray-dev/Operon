//! Mistral AI reasoning models wire format normalization and denormalization.
//!
//! # What is Mistral AI?
//! Mistral AI is a provider of open-source and commercial LLMs. Currently, the
//! Mistral public API does not expose reasoning/thinking traces in its JSON response
//! formats.
//!
//! # How this module handles Mistral:
//! Because there is no wire format for Mistral reasoning content, both
//! [`from_wire_reasoning`] and [`to_wire_reasoning`] will always return a
//! [`ReasoningNormalizeError::NotSupported`] error.
//!
//! This acts as a clean guard to block any attempt to normalize or denormalize
//! reasoning payloads for Mistral, keeping our pipeline deterministic and type-safe.

use serde_json::Value;

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

/// The provider name used in the `NotSupported` error.
const PROVIDER: &str = "Mistral";

/// Normalizes Mistral wire format reasoning content.
///
/// Since Mistral does not expose reasoning/thinking content, this function
/// ALWAYS returns a [`ReasoningNormalizeError::NotSupported`] error.
///
/// # Arguments
/// * `_raw` - The JSON payload from the provider (which we ignore).
///
/// # Returns
/// * Always returns `Err(ReasoningNormalizeError::NotSupported)`.
pub fn from_wire_reasoning(_raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // We immediately return an error because Mistral has no reasoning support.
    // This tells callers that they shouldn't expect thinking blocks from Mistral.
    Err(ReasoningNormalizeError::NotSupported { provider: PROVIDER })
}

/// Denormalizes canonical [`ReasoningBlock`]s back to Mistral wire format.
///
/// Since Mistral does not support reasoning content, this function
/// ALWAYS returns a [`ReasoningNormalizeError::NotSupported`] error.
///
/// # Arguments
/// * `_blocks` - The canonical reasoning blocks we would serialize (ignored).
///
/// # Returns
/// * Always returns `Err(ReasoningNormalizeError::NotSupported)`.
pub fn to_wire_reasoning(_blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // Return an error to prevent trying to serialize reasoning for Mistral.
    Err(ReasoningNormalizeError::NotSupported { provider: PROVIDER })
}
