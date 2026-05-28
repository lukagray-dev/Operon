//! OpenAI wire format normalization and denormalization.
//!
//! OpenAI's tool-call wire format is used directly by OpenAI and also adopted
//! (with minor or no differences) by: DeepSeek, Groq, Mistral, xAI, and Ollama.
//! To avoid code duplication those modules delegate to the `_with_provider`
//! variants of each function, passing their own provider name for error messages.
//!
//! ## Wire formats
//!
//! **Incoming tool call** (inside the `tool_calls` array of an assistant message):
//! ```json
//! {
//!   "id": "call_abc",
//!   "type": "function",
//!   "function": { "name": "read_file", "arguments": "{\"path\":\"/foo\"}" }
//! }
//! ```
//! > Note: `arguments` is a **JSON-encoded string**, NOT a JSON object.
//!
//! **Tool result** (a message with `role: "tool"`):
//! ```json
//! { "role": "tool", "tool_call_id": "call_abc", "name": "read_file", "content": "..." }
//! ```
//!
//! **Tool definition** (inside the `tools` array of a request):
//! ```json
//! { "type": "function", "function": { "name": "read_file", "description": "...", "parameters": {...} } }
//! ```

use serde_json::{json, Value};

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult};

// The name used in error messages when called directly (not via a delegate).
const PROVIDER: &str = "OpenAI";

// ─────────────────────────────────────────────────────────────────────────────
// from_wire — ToolCall
// ─────────────────────────────────────────────────────────────────────────────

/// Parse an OpenAI-compatible tool-call wire value into a canonical [`ToolCall`].
///
/// This is the **shared implementation** for all OpenAI-compatible providers.
/// The `provider_name` parameter is used only in error messages — it does not
/// affect any parsing logic.
///
/// # Errors
/// - [`ToolNormalizeError::MissingField`] if `"id"`, `"function"`, `"function.name"`,
///   or `"function.arguments"` are absent.
/// - [`ToolNormalizeError::ArgumentParseFailed`] if `"function.arguments"` is not
///   valid JSON.
pub fn from_wire_tool_call_with_provider(
    raw: Value,
    provider_name: &'static str,
) -> Result<ToolCall, ToolNormalizeError> {
    // Extract the top-level call ID (e.g. "call_abc123")
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .ok_or(ToolNormalizeError::MissingField {
            field: "id",
            provider: provider_name,
        })?
        .to_string();

    // All function details live under the "function" key
    let function = raw
        .get("function")
        .ok_or(ToolNormalizeError::MissingField {
            field: "function",
            provider: provider_name,
        })?;

    // Extract the function name from inside the "function" object
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .ok_or(ToolNormalizeError::MissingField {
            field: "function.name",
            provider: provider_name,
        })?
        .to_string();

    // The "arguments" field is a JSON-encoded STRING, not a JSON object.
    // Example: "{\"path\":\"/foo\"}" — we MUST parse it with serde_json::from_str.
    // Never assume it is already an object; the OpenAI spec is explicit about this.
    let arguments_str = function
        .get("arguments")
        .and_then(Value::as_str)
        .ok_or(ToolNormalizeError::MissingField {
            field: "function.arguments",
            provider: provider_name,
        })?;

    // Parse the JSON-encoded argument string into an actual serde_json::Value object
    let arguments: Value = serde_json::from_str(arguments_str).map_err(|e| {
        ToolNormalizeError::ArgumentParseFailed {
            provider: provider_name,
            source: e,
        }
    })?;

    Ok(ToolCall {
        id: ToolCallId(id),
        name,
        arguments,
    })
}

/// Parse an OpenAI tool-call wire value into a canonical [`ToolCall`].
///
/// Delegates to [`from_wire_tool_call_with_provider`] with `provider_name = "OpenAI"`.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    from_wire_tool_call_with_provider(raw, PROVIDER)
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolDefinition
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolDefinition`] into the OpenAI-compatible wire format.
///
/// Produces:
/// ```json
/// { "type": "function", "function": { "name": "...", "description": "...", "parameters": {...} } }
/// ```
///
/// This is the **shared implementation** for all OpenAI-compatible providers.
/// The `_provider_name` parameter is reserved for future error reporting and is
/// currently unused (the serialization cannot fail for well-formed types).
pub fn to_wire_tool_definition_with_provider(
    def: &ToolDefinition,
    _provider_name: &'static str,
) -> Result<Value, ToolNormalizeError> {
    // Wrap the definition inside the OpenAI "type: function" envelope
    Ok(json!({
        "type": "function",
        "function": {
            "name": def.name,
            "description": def.description,
            // The JSON Schema parameters object goes in directly — no transformation needed
            "parameters": def.parameters,
        }
    }))
}

/// Serialize a [`ToolDefinition`] into the OpenAI wire format.
///
/// Delegates to [`to_wire_tool_definition_with_provider`] with `provider_name = "OpenAI"`.
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    to_wire_tool_definition_with_provider(def, PROVIDER)
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolResult
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolResult`] into the OpenAI-compatible wire format.
///
/// Produces a `role: "tool"` message:
/// ```json
/// { "role": "tool", "tool_call_id": "call_abc", "name": "read_file", "content": "..." }
/// ```
///
/// The `content` field in OpenAI's format is always a **string**. If the tool
/// returned [`ToolContent::Json`], the JSON value is serialized to a compact
/// string representation so the model can still read it.
///
/// This is the **shared implementation** for all OpenAI-compatible providers.
pub fn to_wire_tool_result_with_provider(
    result: &ToolResult,
    _provider_name: &'static str,
) -> Result<Value, ToolNormalizeError> {
    // OpenAI expects the content as a plain string in the tool result message.
    // For JSON content we serialize it compactly so the model sees valid JSON text.
    let content_str: String = match &result.content {
        ToolContent::Text(s) => s.clone(),
        ToolContent::Json(v) => v.to_string(),
    };

    Ok(json!({
        "role": "tool",
        "tool_call_id": result.call_id.0,
        // OpenAI requires the tool name here so it can correlate message history
        "name": result.name,
        "content": content_str,
    }))
}

/// Serialize a [`ToolResult`] into the OpenAI wire format.
///
/// Delegates to [`to_wire_tool_result_with_provider`] with `provider_name = "OpenAI"`.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    to_wire_tool_result_with_provider(result, PROVIDER)
}
