//! Anthropic wire format normalization and denormalization for reasoning.
//!
//! Anthropic exposes thinking content as a special content block type inside
//! the assistant message's `content` array. The block looks like:
//!
//! ```json
//! { "type": "thinking", "thinking": "I need to...", "signature": "EqoBCkgIAR..." }
//! ```
//!
//! The `"signature"` field is **optional** — it may be absent on some
//! responses (e.g., short contexts, non-function-calling turns), but when
//! present it **must** be echoed back verbatim in subsequent turns to avoid
//! provider-side 4xx errors.
//!
//! ## Normalize input
//! A single content block `Value` (the object above).
//! Returns a `Vec` with exactly one [`ReasoningBlock`].
//!
//! ## Denormalize input
//! A slice of [`ReasoningBlock`]s.
//! Returns a JSON array of `"type":"thinking"` content block objects — one
//! per block. The caller inserts this array into the content array of a
//! user or assistant message.

use serde_json::{json, Value};

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

/// Provider label used in all error messages from this module.
const PROVIDER: &str = "Anthropic";

// ─────────────────────────────────────────────────────────────────────────────
// Normalize — wire → canonical
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a single Anthropic `"thinking"` content block into a canonical
/// [`ReasoningBlock`].
///
/// # Arguments
/// * `raw` — a single content block value, e.g.
///   `{ "type": "thinking", "thinking": "...", "signature": "..." }`
///
/// # Errors
/// - [`ReasoningNormalizeError::MissingField`] with `field = "thinking"` if
///   the `"thinking"` key is absent or not a string.
pub fn from_wire_reasoning(raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // The "thinking" field is required — it carries the actual reasoning text
    let thinking = raw
        .get("thinking")
        .and_then(Value::as_str)
        .ok_or(ReasoningNormalizeError::MissingField {
            field: "thinking",
            provider: PROVIDER,
        })?
        .to_string();

    // The "signature" field is optional — present on most responses but the
    // spec explicitly says it may be absent on some. Never panic or error here.
    let signature = raw
        .get("signature")
        .and_then(Value::as_str)
        .map(str::to_string);

    // Build the canonical block — with or without signature
    let block = match signature {
        Some(sig) => ReasoningBlock::with_signature(thinking, sig),
        None      => ReasoningBlock::new(thinking),
    };

    // Anthropic always produces a single thinking block per content block
    Ok(vec![block])
}

// ─────────────────────────────────────────────────────────────────────────────
// Denormalize — canonical → wire
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a slice of [`ReasoningBlock`]s into an Anthropic wire format
/// content array.
///
/// Returns a JSON array of `"type":"thinking"` content block objects, one per
/// block. The `"signature"` key is **omitted entirely** when the block has no
/// signature — do not include a null; Anthropic expects the key to be absent.
///
/// # Example output
/// ```json
/// [
///   { "type": "thinking", "thinking": "...", "signature": "EqoBCkgIAR..." }
/// ]
/// ```
///
/// # Errors
/// This function is currently infallible but returns `Result` to satisfy the
/// trait contract and remain forward-compatible.
pub fn to_wire_reasoning(blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // Build one Anthropic thinking block per canonical block
    let arr: Vec<Value> = blocks
        .iter()
        .map(|block| {
            // Use an explicit Map so we control which keys appear in the output
            let mut obj = serde_json::Map::new();

            // "type" must be exactly "thinking" — this is how Anthropic
            // distinguishes thinking blocks from text blocks in the array
            obj.insert("type".to_string(), json!("thinking"));

            // The reasoning text itself
            obj.insert("thinking".to_string(), json!(block.thinking));

            // Only include "signature" when the block actually has one.
            // Anthropic requires the key to be *absent* (not null) when
            // there is no signature, so we skip insertion entirely.
            if let Some(sig) = &block.signature {
                obj.insert("signature".to_string(), json!(sig.0));
            }

            Value::Object(obj)
        })
        .collect();

    Ok(Value::Array(arr))
}
