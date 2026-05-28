//! Groq reasoning models wire format normalization and denormalization.
//!
//! # What is Groq?
//! Groq is a high-speed inference engine provider. Currently, the Groq public
//! API does not support exposing reasoning or thinking traces from reasoning
//! models in their JSON payloads.
//!
//! # How this module handles Groq:
//! Because there is no wire format for Groq reasoning content, both
//! [`from_wire_reasoning`] and [`to_wire_reasoning`] will always return a
//! [`ReasoningNormalizeError::NotSupported`] error.
//!
//! Think of this module as a placeholder/guard that explicitly says "No, Groq does not
//! support reasoning!" rather than failing silently or producing garbage data.

use serde_json::Value;

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

/// The provider name used in the `NotSupported` error.
const PROVIDER: &str = "Groq";

/// Normalizes Groq wire format reasoning content.
///
/// Since Groq does not expose reasoning/thinking content, this function
/// ALWAYS returns a [`ReasoningNormalizeError::NotSupported`] error.
///
/// # Arguments
/// * `_raw` - The JSON payload from the provider (which we ignore).
///
/// # Returns
/// * Always returns `Err(ReasoningNormalizeError::NotSupported)`.
pub fn from_wire_reasoning(_raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // We immediately return an error because Groq has no reasoning support.
    // This lets callers know they shouldn't expect thinking blocks from Groq.
    Err(ReasoningNormalizeError::NotSupported { provider: PROVIDER })
}

/// Denormalizes canonical [`ReasoningBlock`]s back to Groq wire format.
///
/// Since Groq does not support reasoning content, this function
/// ALWAYS returns a [`ReasoningNormalizeError::NotSupported`] error.
///
/// # Arguments
/// * `_blocks` - The canonical reasoning blocks we would serialize (ignored).
///
/// # Returns
/// * Always returns `Err(ReasoningNormalizeError::NotSupported)`.
pub fn to_wire_reasoning(_blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // Return an error to prevent sending unsupported fields to Groq.
    Err(ReasoningNormalizeError::NotSupported { provider: PROVIDER })
}
