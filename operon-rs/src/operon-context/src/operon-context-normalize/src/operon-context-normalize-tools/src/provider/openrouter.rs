//! OpenRouter wire format normalization and denormalization.
//!
//! OpenRouter is a gateway that proxies requests to multiple underlying providers.
//! It passes through the underlying provider's wire format — which can be either
//! **OpenAI-style** or **Anthropic-style** depending on the model the user selected.
//!
//! ## Shape detection
//!
//! Because this crate does not have access to HTTP response headers (where OpenRouter
//! includes the `X-Openrouter-Provider` header), shape detection is done purely from
//! the raw JSON keys:
//!
//! | Condition | Detected shape |
//! |---|---|
//! | JSON object has a `"function"` key | OpenAI style |
//! | JSON object has `"type"` = `"tool_use"` | Anthropic style |
//! | Neither | [`ToolNormalizeError::UnknownShape`] |
//!
//! **Important:** Detection is done on key presence, NOT by attempting a parse and
//! catching errors. This avoids masking real parsing failures.
//!
//! ## Wire formats
//!
//! - **OpenAI style**: see [`openai`](super::openai)
//! - **Anthropic style**: see [`anthropic`](super::anthropic)
//!
//! For `denormalize_*`, OpenRouter accepts OpenAI-compatible format in both cases
//! (this is the format the OpenRouter API itself documents for sending tool definitions
//! and results upstream).

use serde_json::Value;

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolDefinition, ToolResult};

use super::{anthropic, openai};

const PROVIDER: &str = "OpenRouter";

// ─────────────────────────────────────────────────────────────────────────────
// from_wire — ToolCall
// ─────────────────────────────────────────────────────────────────────────────

/// Parse an OpenRouter tool-call wire value into a canonical [`ToolCall`].
///
/// Detects the underlying provider shape from the raw JSON key set:
/// - A `"function"` key → delegates to the OpenAI parser.
/// - `"type"` = `"tool_use"` → delegates to the Anthropic parser.
/// - Anything else → returns [`ToolNormalizeError::UnknownShape`].
///
/// Detection is done purely on key presence — no speculative parsing, no error
/// catching. This means a real parsing failure (bad arguments string) will surface
/// as the correct error variant from the underlying parser.
///
/// # Errors
/// - [`ToolNormalizeError::UnknownShape`] if neither shape is detected.
/// - Propagates any error from the underlying OpenAI or Anthropic parser.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    // Detect shape from key presence first — do NOT try/catch parse errors
    if raw.get("function").is_some() {
        // OpenAI shape: has a "function" key at the top level
        // Pass "OpenRouter" as the provider name so errors say "OpenRouter" not "OpenAI"
        openai::from_wire_tool_call_with_provider(raw, PROVIDER)
    } else if raw.get("type").and_then(Value::as_str) == Some("tool_use") {
        // Anthropic shape: has "type": "tool_use" at the top level
        anthropic::from_wire_tool_call(raw)
    } else {
        // Could not determine shape — collect key names for a helpful error message
        let found_keys: Vec<String> = raw
            .as_object()
            .map(|obj| obj.keys().cloned().collect())
            .unwrap_or_default();

        Err(ToolNormalizeError::UnknownShape {
            provider: PROVIDER,
            detail: format!(
                "expected a 'function' key (OpenAI shape) or 'type':'tool_use' \
                 (Anthropic shape), but found keys: {:?}",
                found_keys
            ),
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolDefinition
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolDefinition`] into the OpenRouter wire format.
///
/// OpenRouter's own documentation specifies the OpenAI-compatible tool definition
/// format for sending tool descriptions in requests. Delegates to [`openai`].
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_definition_with_provider(def, PROVIDER)
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolResult
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolResult`] into the OpenRouter wire format.
///
/// OpenRouter accepts OpenAI-compatible `role: "tool"` messages for sending
/// tool results back into context. Delegates to [`openai`].
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_result_with_provider(result, PROVIDER)
}
