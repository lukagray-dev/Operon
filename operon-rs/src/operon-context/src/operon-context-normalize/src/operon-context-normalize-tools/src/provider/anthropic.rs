//! Anthropic wire format normalization and denormalization.
//!
//! Anthropic's tool-call wire format is structurally different from OpenAI's:
//! arguments are passed as a parsed JSON object under `"input"` (not as a
//! JSON-encoded string), and the call ID field is named `"id"` at the top level.
//!
//! ## Wire formats
//!
//! **Incoming tool call** (inside the `content` array of an assistant message):
//! ```json
//! { "type": "tool_use", "id": "toolu_01A", "name": "read_file", "input": { "path": "/foo" } }
//! ```
//!
//! **Tool result** (inside the `content` array of a user message):
//! ```json
//! {
//!   "type": "tool_result",
//!   "tool_use_id": "toolu_01A",
//!   "content": "file contents here",
//!   "is_error": false
//! }
//! ```
//!
//! **Tool definition** (inside the `tools` array of a request):
//! ```json
//! { "name": "read_file", "description": "...", "input_schema": { "type": "object", ... } }
//! ```
//! > Note: Anthropic uses `"input_schema"` where OpenAI uses `"parameters"`.

use serde_json::{json, Value};

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult};

const PROVIDER: &str = "Anthropic";

// ─────────────────────────────────────────────────────────────────────────────
// from_wire — ToolCall
// ─────────────────────────────────────────────────────────────────────────────

/// Parse an Anthropic tool-call wire value into a canonical [`ToolCall`].
///
/// Expects a `"tool_use"` content block as produced by the Anthropic Messages API.
/// Unlike OpenAI, Anthropic transmits arguments under `"input"` as a real JSON
/// object — no string parsing step is needed.
///
/// # Errors
/// - [`ToolNormalizeError::MissingField`] if `"id"`, `"name"`, or `"input"` are absent.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    // Extract the call ID, e.g. "toolu_01A02B03C"
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .ok_or(ToolNormalizeError::MissingField {
            field: "id",
            provider: PROVIDER,
        })?
        .to_string();

    // Extract the tool name from the top-level "name" field
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .ok_or(ToolNormalizeError::MissingField {
            field: "name",
            provider: PROVIDER,
        })?
        .to_string();

    // Anthropic uses "input" (not "arguments") and it is already a parsed JSON object.
    // No from_str call needed — just clone the Value directly.
    let arguments = raw
        .get("input")
        .ok_or(ToolNormalizeError::MissingField {
            field: "input",
            provider: PROVIDER,
        })?
        .clone();

    Ok(ToolCall {
        id: ToolCallId(id),
        name,
        arguments,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolDefinition
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolDefinition`] into the Anthropic wire format.
///
/// Produces:
/// ```json
/// { "name": "...", "description": "...", "input_schema": { "type": "object", ... } }
/// ```
///
/// The key difference from OpenAI is that the parameter schema goes under
/// `"input_schema"` rather than `"parameters"`, and there is no outer
/// `"type": "function"` envelope.
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    Ok(json!({
        "name": def.name,
        "description": def.description,
        // Anthropic names this field "input_schema" rather than "parameters"
        "input_schema": def.parameters,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolResult
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolResult`] into the Anthropic wire format.
///
/// Produces a `"tool_result"` content block for inclusion in a user message:
/// ```json
/// { "type": "tool_result", "tool_use_id": "toolu_01A", "content": "...", "is_error": false }
/// ```
///
/// Anthropic's `"content"` field accepts a string or an array of content blocks.
/// For simplicity and maximum compatibility this implementation always serializes
/// to a string: [`ToolContent::Text`] passes through directly, and
/// [`ToolContent::Json`] is converted to its compact JSON string representation.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    // Serialize the content to a string (Anthropic accepts string content here)
    let content_value: Value = match &result.content {
        ToolContent::Text(s) => Value::String(s.clone()),
        // Serialize the JSON value to a compact string so Anthropic can embed it
        ToolContent::Json(v) => Value::String(v.to_string()),
    };

    Ok(json!({
        "type": "tool_result",
        // Anthropic uses "tool_use_id" to reference the original tool_use block
        "tool_use_id": result.call_id.0,
        "content": content_value,
        // is_error lets Anthropic know the execution failed and reason accordingly
        "is_error": result.is_error,
    }))
}
