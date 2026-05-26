//! Google Gemini reasoning/thinking wire format normalization and
//! denormalization.
//!
//! Gemini 2.5+ and 3 series expose thinking content as special "thought parts"
//! inside the `parts` array of a model content block. A thought part is
//! identified by `"thought": true` on the part object.
//!
//! ```json
//! { "text": "I need to approach this problem by...", "thought": true, "thoughtSignature": "AUBSKg..." }
//! ```
//!
//! The `"thoughtSignature"` field uses **camelCase** exactly as the Gemini API
//! specifies — do not normalize it to snake_case.
//!
//! The signature is:
//! - **Optional** on Gemini 2.5 responses (older models may omit it).
//! - **Required** on Gemini 3 responses when function calling is involved.
//!   Omitting it causes provider-side 4xx errors.
//!
//! **Important:** The caller is responsible for filtering thought parts from
//! regular text parts before calling [`from_wire_reasoning`]. This function
//! normalizes whatever part is passed — it does not assert `"thought": true`.
//!
//! ## Normalize input
//! A single thought part `Value` — the object above.
//! Returns a `Vec` with exactly one [`ReasoningBlock`].
//!
//! ## Denormalize input
//! A slice of [`ReasoningBlock`]s.
//! Returns a JSON array of Gemini thought part objects — one per block.
//! The `"thoughtSignature"` key is **omitted** when the block has no signature.

use serde_json::{json, Value};

use crate::error::ReasoningNormalizeError;
use crate::types::ReasoningBlock;

/// Provider label used in all error messages from this module.
const PROVIDER: &str = "Gemini";

// ─────────────────────────────────────────────────────────────────────────────
// Normalize — wire → canonical
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a single Gemini thought part into a canonical [`ReasoningBlock`].
///
/// # Arguments
/// * `raw` — a single thought part, e.g.
///   `{ "text": "...", "thought": true, "thoughtSignature": "..." }`
///
/// # Errors
/// - [`ReasoningNormalizeError::MissingField`] with `field = "text"` if the
///   `"text"` key is absent or not a string.
pub fn from_wire_reasoning(raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // "text" is required — it contains the thinking content.
    // A thought part without "text" is malformed and cannot be normalized.
    let thinking = raw
        .get("text")
        .and_then(Value::as_str)
        .ok_or(ReasoningNormalizeError::MissingField {
            field: "text",
            provider: PROVIDER,
        })?
        .to_string();

    // "thoughtSignature" is optional — camelCase exactly as the API specifies.
    // Present on Gemini 3 function-calling responses; absent on older 2.5 ones.
    let signature = raw
        .get("thoughtSignature")
        .and_then(Value::as_str)
        .map(str::to_string);

    // Build canonical block — with or without the opaque signature token
    let block = match signature {
        Some(sig) => ReasoningBlock::with_signature(thinking, sig),
        None      => ReasoningBlock::new(thinking),
    };

    Ok(vec![block])
}

// ─────────────────────────────────────────────────────────────────────────────
// Denormalize — canonical → wire
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a slice of [`ReasoningBlock`]s into a Gemini thought parts array.
///
/// Returns a JSON array of thought part objects — one per block:
/// ```json
/// [{ "text": "...", "thought": true, "thoughtSignature": "AUBSKg..." }]
/// ```
///
/// The `"thoughtSignature"` key is **omitted entirely** (not set to null) when
/// the block has no signature. Gemini requires the key to be absent in that
/// case.
///
/// # Errors
/// This function is currently infallible but returns `Result` for trait
/// compatibility and forward-compatibility.
pub fn to_wire_reasoning(blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // Build one Gemini thought part per canonical block
    let arr: Vec<Value> = blocks
        .iter()
        .map(|block| {
            // Use an explicit Map so we fully control which keys appear and
            // in what order — important for keeping the wire format clean
            let mut obj = serde_json::Map::new();

            // The thinking text goes in the standard "text" field
            obj.insert("text".to_string(), json!(block.thinking));

            // "thought": true is the marker that distinguishes a thought part
            // from a regular text part in the Gemini parts array
            obj.insert("thought".to_string(), json!(true));

            // "thoughtSignature" uses camelCase — MUST match the API exactly.
            // Only include when the block has a signature; omit otherwise.
            if let Some(sig) = &block.signature {
                obj.insert("thoughtSignature".to_string(), json!(sig.0));
            }

            Value::Object(obj)
        })
        .collect();

    Ok(Value::Array(arr))
}
