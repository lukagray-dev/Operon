//! OpenAI o-series reasoning models wire format normalization and
//! denormalization.
//!
//! OpenAI reasoning models (o1, o3, o4) expose their chain-of-thought via a
//! `"reasoning_summary"` top-level array in the response. Each element is a
//! `summary_text` block:
//!
//! ```json
//! {
//!   "reasoning_summary": [
//!     { "type": "summary_text", "text": "The user is asking about..." },
//!     { "type": "summary_text", "text": "I should approach this by..." }
//!   ]
//! }
//! ```
//!
//! There is no signature concept for OpenAI reasoning — the blocks carry
//! only text.
//!
//! ## Normalize input
//! The full `reasoning_summary` array `Value` (not the enclosing response
//! object). Each element maps to one [`ReasoningBlock`].
//!
//! ## Denormalize input
//! A slice of [`ReasoningBlock`]s.
//! Returns a `reasoning_summary` array `Value` — a JSON array of
//! `{ "type": "summary_text", "text": "..." }` objects.

use serde_json::{json, Value};

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

/// Provider label used in all error messages from this module.
const PROVIDER: &str = "OpenAI";

// ─────────────────────────────────────────────────────────────────────────────
// Normalize — wire → canonical
// ─────────────────────────────────────────────────────────────────────────────

/// Parse the OpenAI `reasoning_summary` array into canonical
/// [`ReasoningBlock`]s.
///
/// # Arguments
/// * `raw` — the `reasoning_summary` array value, e.g.
///   `[{ "type": "summary_text", "text": "..." }, ...]`
///
/// # Errors
/// - [`ReasoningNormalizeError::MissingField`] if `raw` is not a JSON array
///   (caller passed the wrong value).
/// - [`ReasoningNormalizeError::EmptyReasoningSummary`] if the array is empty.
/// - [`ReasoningNormalizeError::MissingField`] with `field = "text"` if any
///   element lacks a `"text"` string field.
pub fn from_wire_reasoning(raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // The caller must pass the reasoning_summary array directly — not the
    // enclosing response object
    let arr = match raw {
        Value::Array(a) => a,
        _ => {
            return Err(ReasoningNormalizeError::MissingField {
                field: "reasoning_summary",
                provider: PROVIDER,
            })
        }
    };

    // An empty array means the model produced no usable reasoning content
    if arr.is_empty() {
        return Err(ReasoningNormalizeError::EmptyReasoningSummary { provider: PROVIDER });
    }

    // Extract the "text" field from each summary element — one block per element
    let blocks = arr
        .into_iter()
        .map(|elem| {
            // Each element must have a "text" string field
            let text = elem
                .get("text")
                .and_then(Value::as_str)
                .ok_or(ReasoningNormalizeError::MissingField {
                    field: "text",
                    provider: PROVIDER,
                })?
                .to_string();

            // OpenAI reasoning blocks carry no signature
            Ok(ReasoningBlock::new(text))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(blocks)
}

// ─────────────────────────────────────────────────────────────────────────────
// Denormalize — canonical → wire
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a slice of [`ReasoningBlock`]s into an OpenAI
/// `reasoning_summary` array.
///
/// Returns a JSON array of `{ "type": "summary_text", "text": "..." }`
/// objects — one per block. The caller uses this as the value for the
/// `"reasoning_summary"` key in the request.
///
/// # Errors
/// This function is currently infallible but returns `Result` to satisfy the
/// trait contract.
pub fn to_wire_reasoning(blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // One summary_text element per canonical block — no signature to include
    let arr: Vec<Value> = blocks
        .iter()
        .map(|block| {
            json!({
                "type": "summary_text",
                "text": block.thinking,
            })
        })
        .collect();

    Ok(Value::Array(arr))
}
