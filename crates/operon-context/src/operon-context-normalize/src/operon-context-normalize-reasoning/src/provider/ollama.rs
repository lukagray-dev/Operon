//! Ollama native reasoning wire format normalization and denormalization.
//!
//! # What is Ollama Native Thinking?
//! Ollama's native `/api/chat` format supports returning the model's thinking trace.
//! Thinking is opt-in via `"think": true` in the request parameters, and the response
//! contains a `"thinking"` string field on the assistant message object:
//!
//! ```json
//! {
//!   "role": "assistant",
//!   "content": "Here is the final answer...",
//!   "thinking": "I need to analyze this step by step..."
//! }
//! ```
//!
//! Note that Ollama's OpenAI-compatible endpoint `/v1/chat/completions` uses the DeepSeek
//! path (i.e., `"reasoning_content"`). This module specifically targets the native
//! `/api/chat` endpoint's `"thinking"` field format.
//!
//! The caller extracts the `"thinking"` string field and passes **only that value**
//! (a `Value::String`) to [`from_wire_reasoning`].
//!
//! There is no signature concept for Ollama thinking content.
//!
//! ## Normalize input
//! The `"thinking"` field value as a [`serde_json::Value::String`].
//! Returns a `Vec` with exactly one canonical [`ReasoningBlock`] with no signature.
//!
//! ## Denormalize output
//! A [`serde_json::Value::String`] containing the thinking text.

use serde_json::Value;

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

/// The provider name used in error messages.
const PROVIDER: &str = "Ollama";

/// Parse Ollama's native `"thinking"` string value into a canonical [`ReasoningBlock`].
///
/// # Arguments
/// * `raw` - The JSON value of the `"thinking"` field. Must be a string.
///
/// # Errors
/// - [`ReasoningNormalizeError::MissingField`] if `raw` is not a string.
pub fn from_wire_reasoning(raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // Ollama's thinking field must be a JSON string. If it's anything else (like null or an object),
    // we return a MissingField error referencing the expected field "thinking".
    let text = match raw {
        Value::String(s) => s,
        _ => {
            return Err(ReasoningNormalizeError::MissingField {
                field: "thinking",
                provider: PROVIDER,
            })
        }
    };

    // Return a single block containing the thinking trace. No signature is used.
    Ok(vec![ReasoningBlock::new(text)])
}

/// Serialize a slice of [`ReasoningBlock`]s into an Ollama native thinking string value.
///
/// If multiple blocks are provided, their thinking text will be joined by double newlines.
///
/// # Arguments
/// * `blocks` - A slice of canonical reasoning blocks to serialize.
///
/// # Returns
/// * A `Value::String` containing the thinking text.
pub fn to_wire_reasoning(blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // If the caller passes multiple blocks, we join them together using double newlines.
    // In typical usage, there is only one block.
    let text = blocks
        .iter()
        .map(|b| b.thinking.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(Value::String(text))
}
