//! OpenRouter wire format normalization and denormalization.
//!
//! # What is OpenRouter?
//! OpenRouter is an API gateway that proxies requests to many different underlying LLMs.
//! It passes through the raw thinking formats returned by the underlying providers.
//!
//! # Shape Detection
//! Because we do not have access to HTTP headers (like `X-Openrouter-Provider`) when
//! normalizing, we must determine the underlying provider's format by inspecting the
//! JSON keys and structure:
//!
//! 1. **String** (`Value::String`) -> DeepSeek / xAI / Ollama shape (a plain thinking string).
//! 2. **Array** (`Value::Array`) -> OpenAI shape (an array of `summary_text` blocks).
//! 3. **Object** (`Value::Object`) ->
//!    - If `"type"` is `"thinking"` -> Anthropic shape.
//!    - If `"thought"` is `true` -> Gemini shape.
//!    - Otherwise -> [`ReasoningNormalizeError::UnknownShape`].
//! 4. **Other** -> [`ReasoningNormalizeError::UnknownShape`].
//!
//! OpenRouter API documentation specifies that the OpenAI-style `reasoning_summary` format
//! (a JSON array of summary blocks) should be used when sending reasoning back in request turns.
//! Therefore, [`to_wire_reasoning`] always outputs the OpenAI-style array structure.

use serde_json::Value;

use crate::error::ReasoningNormalizeError;
use crate::provider::deepseek;
use crate::types::ReasoningBlock;

/// The provider name used in all error messages in this module.
const PROVIDER: &str = "OpenRouter";

/// Parse an OpenRouter reasoning payload into canonical [`ReasoningBlock`]s.
///
/// Detects the underlying provider shape from the structure and keys of the JSON payload.
///
/// # Arguments
/// * `raw` - The JSON payload representing the reasoning/thinking content.
///
/// # Errors
/// - [`ReasoningNormalizeError::UnknownShape`] if the shape cannot be detected.
/// - [`ReasoningNormalizeError::MissingField`] or other parsing errors if the detected shape is malformed.
pub fn from_wire_reasoning(raw: Value) -> Result<Vec<ReasoningBlock>, ReasoningNormalizeError> {
    // 1. Detect String (DeepSeek, xAI, Ollama shape)
    if raw.is_string() {
        // DeepSeek/xAI/Ollama shape: raw string of thinking content.
        // We reuse the DeepSeek helper but pass "OpenRouter" as the provider.
        return deepseek::from_wire_reasoning_with_provider(raw, PROVIDER);
    }

    // 2. Detect Array (OpenAI shape)
    if raw.is_array() {
        // OpenAI shape: array of summary_text blocks.
        // We parse it manually here to ensure any errors mention "OpenRouter" instead of "OpenAI".
        let arr = raw
            .as_array()
            .ok_or(ReasoningNormalizeError::MissingField {
                field: "reasoning_summary",
                provider: PROVIDER,
            })?;

        if arr.is_empty() {
            return Err(ReasoningNormalizeError::EmptyReasoningSummary { provider: PROVIDER });
        }

        let blocks = arr
            .iter()
            .map(|elem| {
                let text = elem
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(ReasoningNormalizeError::MissingField {
                        field: "text",
                        provider: PROVIDER,
                    })?
                    .to_string();
                Ok(ReasoningBlock::new(text))
            })
            .collect::<Result<Vec<_>, _>>()?;

        return Ok(blocks);
    }

    // 3. Detect Object (Anthropic or Gemini shape)
    if raw.is_object() {
        // Look for Anthropic type: "thinking" block
        if raw.get("type").and_then(Value::as_str) == Some("thinking") {
            let thinking = raw
                .get("thinking")
                .and_then(Value::as_str)
                .ok_or(ReasoningNormalizeError::MissingField {
                    field: "thinking",
                    provider: PROVIDER,
                })?
                .to_string();

            let signature = raw
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_string);

            let block = match signature {
                Some(sig) => ReasoningBlock::with_signature(thinking, sig),
                None => ReasoningBlock::new(thinking),
            };

            return Ok(vec![block]);
        }

        // Look for Gemini thought: true block
        if raw.get("thought").and_then(Value::as_bool) == Some(true) {
            let thinking = raw
                .get("text")
                .and_then(Value::as_str)
                .ok_or(ReasoningNormalizeError::MissingField {
                    field: "text",
                    provider: PROVIDER,
                })?
                .to_string();

            let signature = raw
                .get("thoughtSignature")
                .and_then(Value::as_str)
                .map(str::to_string);

            let block = match signature {
                Some(sig) => ReasoningBlock::with_signature(thinking, sig),
                None => ReasoningBlock::new(thinking),
            };

            return Ok(vec![block]);
        }

        // Object has some other fields we don't recognize
        let found_keys: Vec<String> = raw
            .as_object()
            .map(|obj| obj.keys().cloned().collect())
            .unwrap_or_default();

        return Err(ReasoningNormalizeError::UnknownShape {
            provider: PROVIDER,
            detail: format!(
                "JSON object did not match Anthropic shape (expected 'type':'thinking') \
                 or Gemini shape (expected 'thought':true). Found keys: {:?}",
                found_keys
            ),
        });
    }

    // 4. Other types (Boolean, Number, Null)
    Err(ReasoningNormalizeError::UnknownShape {
        provider: PROVIDER,
        detail: format!("expected string, array, or object, but found: {}", raw),
    })
}

/// Serialize a slice of [`ReasoningBlock`]s into OpenRouter wire format.
///
/// OpenRouter specifies that reasoning outputs should be denormalized into the
/// OpenAI-compatible `reasoning_summary` array structure (i.e. a JSON array of
/// `{ "type": "summary_text", "text": "..." }` objects).
///
/// # Arguments
/// * `blocks` - A slice of canonical reasoning blocks to serialize.
///
/// # Returns
/// * A `Value::Array` containing the OpenAI-compatible objects.
pub fn to_wire_reasoning(blocks: &[ReasoningBlock]) -> Result<Value, ReasoningNormalizeError> {
    // OpenRouter always maps blocks to OpenAI summary_text format
    let arr: Vec<Value> = blocks
        .iter()
        .map(|block| {
            serde_json::json!({
                "type": "summary_text",
                "text": block.thinking,
            })
        })
        .collect();

    Ok(Value::Array(arr))
}
