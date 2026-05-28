//! DeepSeek reasoning models wire format normalization and denormalization.
//!
//! DeepSeek exposes its chain-of-thought via a `"reasoning_content"` string
//! field on the assistant message object (distinct from `"content"` which
//! holds the final answer):
//!
//! ```json
//! {
//!   "role": "assistant",
//!   "content": "The answer is 42",
//!   "reasoning_content": "First let me think step by step..."
//! }
//! ```
//!
//! The caller extracts the `"reasoning_content"` string value and passes
//! **only that value** (a `Value::String`) to [`from_wire_reasoning`] — not
//! the whole message object.
//!
//! There is no signature concept for DeepSeek reasoning.
//!
//! ## Normalize input
//! The `reasoning_content` field value as a `Value::String`.
//! Returns a `Vec` with exactly one [`ReasoningBlock`], no signature.
//!
//! ## Denormalize output
//! A `Value::String` of the thinking text.

use serde_json::Value;

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

/// Provider label used in all error messages from this module.
///
/// xAI delegates to this module's logic but overrides this with its own
/// `"xAI"` label via [`from_wire_reasoning_with_provider`] — so error
/// messages always reference the correct provider name.
const PROVIDER: &str = "DeepSeek";

// ─────────────────────────────────────────────────────────────────────────────
// Normalize — wire → canonical
// ─────────────────────────────────────────────────────────────────────────────

/// Parse the DeepSeek `reasoning_content` string value into a canonical
/// [`ReasoningBlock`].
///
/// Delegates to [`from_wire_reasoning_with_provider`] using `"DeepSeek"` as
/// the provider label in any error messages.
pub fn from_wire_reasoning(raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    from_wire_reasoning_with_provider(raw, PROVIDER)
}

/// Core normalize logic shared with xAI.
///
/// Accepts a `provider` label so that xAI can delegate here while keeping
/// its own name in error messages (the spec requires xAI errors to say
/// `"xAI"`, not `"DeepSeek"`).
///
/// # Arguments
/// * `raw` — the `reasoning_content` value — must be a `Value::String`.
/// * `provider` — the provider label for error messages.
///
/// # Errors
/// - [`ReasoningNormalizeError::MissingField`] if `raw` is null, an object,
///   or any non-string JSON value.
pub fn from_wire_reasoning_with_provider(
    raw: Value,
    provider: &'static str,
) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // reasoning_content must be a plain string — null or any other type is
    // considered a missing/invalid field (the caller should only invoke this
    // when they know the field was present and non-null)
    let text = match raw {
        Value::String(s) => s,
        _ => {
            return Err(ReasoningNormalizeError::MissingField {
                field: "reasoning_content",
                provider,
            })
        }
    };

    // DeepSeek always produces a single reasoning string — wrap it in a Vec
    // of one block with no signature
    Ok(vec![ReasoningBlock::new(text)])
}

// ─────────────────────────────────────────────────────────────────────────────
// Denormalize — canonical → wire
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a slice of [`ReasoningBlock`]s back to a DeepSeek
/// `reasoning_content` string value.
///
/// Returns a `Value::String`. If multiple blocks are present (unusual for
/// DeepSeek, which always produces one), their thinking texts are joined with
/// a double newline to preserve readability.
///
/// Delegates to [`to_wire_reasoning_with_provider`] using `"DeepSeek"`.
pub fn to_wire_reasoning(blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    to_wire_reasoning_with_provider(blocks, PROVIDER)
}

/// Core denormalize logic shared with xAI.
///
/// Accepts a `provider` label for symmetric API with the normalize side.
/// Currently the label is not used (the serialization cannot fail), but it
/// is kept for forward-compatibility with future `SerializeFailed` paths.
pub fn to_wire_reasoning_with_provider(
    blocks: &[ReasoningBlock],
    _provider: &'static str,
) -> Result<Value, ReasoningNormalizeError> {
    // If the caller somehow passes multiple blocks (edge case), join them.
    // In normal usage there will always be exactly one block for DeepSeek/xAI.
    let text = blocks
        .iter()
        .map(|b| b.thinking.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(Value::String(text))
}
