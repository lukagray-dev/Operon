//! Cohere reasoning models wire format normalization and denormalization.
//!
//! # What is Cohere?
//! Cohere is a provider of enterprise LLMs (Command R, Command R+, etc.).
//! Currently, Cohere's API does not expose reasoning/thinking traces in its JSON
//! payload formats.
//!
//! # How this module handles Cohere:
//! Because there is no wire format for Cohere reasoning content, both
//! [`from_wire_reasoning`] and [`to_wire_reasoning`] will always return a
//! [`ReasoningNormalizeError::NotSupported`] error.
//!
//! This acts as a clean guard to block any attempt to normalize or denormalize
//! reasoning payloads for Cohere, keeping the pipeline deterministic and type-safe.

use serde_json::Value;

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

/// The provider name used in the `NotSupported` error.
const PROVIDER: &str = "Cohere";

/// Normalizes Cohere wire format reasoning content.
///
/// Since Cohere does not expose reasoning/thinking content, this function
/// ALWAYS returns a [`ReasoningNormalizeError::NotSupported`] error.
///
/// # Arguments
/// * `_raw` - The JSON payload from the provider (which we ignore).
///
/// # Returns
/// * Always returns `Err(ReasoningNormalizeError::NotSupported)`.
pub fn from_wire_reasoning(_raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // We immediately return an error because Cohere has no reasoning support.
    // This tells callers that they shouldn't expect thinking blocks from Cohere.
    Err(ReasoningNormalizeError::NotSupported { provider: PROVIDER })
}

/// Denormalizes canonical [`ReasoningBlock`]s back to Cohere wire format.
///
/// Since Cohere does not support reasoning content, this function
/// ALWAYS returns a [`ReasoningNormalizeError::NotSupported`] error.
///
/// # Arguments
/// * `_blocks` - The canonical reasoning blocks we would serialize (ignored).
///
/// # Returns
/// * Always returns `Err(ReasoningNormalizeError::NotSupported)`.
pub fn to_wire_reasoning(_blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // Return an error to prevent trying to serialize reasoning for Cohere.
    Err(ReasoningNormalizeError::NotSupported { provider: PROVIDER })
}
