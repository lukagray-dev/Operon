//! xAI Grok reasoning models wire format normalization and denormalization.
//!
//! # What is xAI / Grok Reasoning?
//! xAI's reasoning models (e.g. Grok-2, Grok-3, etc.) return reasoning/thinking traces in
//! the same structure as DeepSeek. The reasoning text is returned in a top-level string field
//! named `"reasoning_content"` inside the assistant message object:
//!
//! ```json
//! {
//!   "role": "assistant",
//!   "content": "Here is the final answer...",
//!   "reasoning_content": "Let me process the input carefully step-by-step..."
//! }
//! ```
//!
//! # How this module works:
//! Because the wire structure is exactly the same as DeepSeek, we delegate all parsing and
//! serialization logic to [`crate::provider::deepseek`]. However, to meet our diagnostic
//! requirements, we override the provider label to `"xAI"`. This ensures any generated error
//! messages (like a missing field error) correctly state `"xAI"` rather than confusingly
//! mentioning `"DeepSeek"`.
//!
//! There is no signature concept for xAI reasoning content.
//!
//! ## Normalize input
//! The `"reasoning_content"` string field value as a [`serde_json::Value::String`].
//! Returns a `Vec` with exactly one canonical [`ReasoningBlock`] with no signature.
//!
//! ## Denormalize output
//! A [`serde_json::Value::String`] containing the joined thinking text.

use serde_json::Value;

use crate::error::ReasoningNormalizeError;
use crate::provider::deepseek;
use crate::types::ReasoningBlock;

/// The provider name used in errors for this module.
const PROVIDER: &str = "xAI";

/// Parse xAI's `reasoning_content` string value into a canonical [`ReasoningBlock`].
///
/// Delegates directly to the DeepSeek implementation using `"xAI"` as the provider label.
///
/// # Arguments
/// * `raw` - The JSON payload representing the `reasoning_content` field (must be a JSON string).
///
/// # Errors
/// - [`ReasoningNormalizeError::MissingField`] if `raw` is not a string.
pub fn from_wire_reasoning(raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // Forward directly to DeepSeek's shared implementation, but specifying "xAI"
    // as the provider label for error diagnostics.
    deepseek::from_wire_reasoning_with_provider(raw, PROVIDER)
}

/// Serialize a slice of [`ReasoningBlock`]s into an xAI `reasoning_content` string value.
///
/// Delegates directly to the DeepSeek implementation using `"xAI"` as the provider label.
///
/// # Arguments
/// * `blocks` - A slice of canonical reasoning blocks to serialize.
///
/// # Returns
/// * A `serde_json::Value::String` containing the thinking text of all blocks joined by double newlines.
pub fn to_wire_reasoning(blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // Forward directly to DeepSeek's shared implementation, specifying "xAI".
    deepseek::to_wire_reasoning_with_provider(blocks, PROVIDER)
}
