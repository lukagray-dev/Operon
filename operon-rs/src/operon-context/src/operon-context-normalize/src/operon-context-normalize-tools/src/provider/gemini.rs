//! Google Gemini wire format normalization and denormalization.
//!
//! Gemini's tool-call shape is substantially different from OpenAI/Anthropic:
//! - Arguments live under `"functionCall.args"` as a real JSON object (no string encoding).
//! - **Gemini provides no call ID** in the wire format. This crate generates a
//!   deterministic synthetic ID by hashing the function name and serialized args with
//!   `std::collections::hash_map::DefaultHasher`, formatted as `"gemini-{:016x}"`.
//!   The same inputs always produce the same ID, which is required to pair results.
//! - Tool definitions are wrapped in `"function_declarations"` arrays.
//!
//! ## Wire formats
//!
//! **Incoming tool call** (inside the `parts` array of a model message):
//! ```json
//! { "functionCall": { "name": "read_file", "args": { "path": "/foo" } } }
//! ```
//!
//! **Tool result** (inside the `parts` array of a `role: "user"` message):
//! ```json
//! { "functionResponse": { "name": "read_file", "response": { "content": "..." } } }
//! ```
//!
//! **Tool definition** (a `tools` entry):
//! ```json
//! { "function_declarations": [{ "name": "read_file", "description": "...", "parameters": {...} }] }
//! ```

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde_json::{json, Value};

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult};

const PROVIDER: &str = "Gemini";

// ─────────────────────────────────────────────────────────────────────────────
// ID generation — deterministic, std-only, no external crates
// ─────────────────────────────────────────────────────────────────────────────

/// Generate a deterministic synthetic call ID for a Gemini tool call.
///
/// Gemini's `functionCall` wire format does not include a unique call ID, but the
/// rest of the crate (and your application) needs one to match each call with its
/// result. We therefore create a stable ID by hashing the function name and a
/// canonical string representation of its arguments.
///
/// Uses only `std::collections::hash_map::DefaultHasher` — no `uuid` crate, no
/// `sha2` crate, no external deps at all. The output format is `"gemini-{:016x}"`,
/// e.g. `"gemini-a3f2b1c4d5e6f7a8"`.
///
/// # Stability note
/// `DefaultHasher`'s algorithm is not guaranteed to be stable across Rust versions,
/// but it is stable within a single binary execution, which is sufficient for the
/// purpose of pairing a call with its result within one request/response cycle.
fn generate_gemini_id(name: &str, args: &Value) -> String {
    let mut hasher = DefaultHasher::new();

    // Hash the function name first — two tools with the same args but different
    // names must produce different IDs
    name.hash(&mut hasher);

    // Hash a canonical string representation of the args object.
    // serde_json's Display output is deterministic for the same Value.
    args.to_string().hash(&mut hasher);

    let hash_value = hasher.finish();

    // Format as zero-padded 16-character lowercase hex with a human-readable prefix
    format!("gemini-{:016x}", hash_value)
}

// ─────────────────────────────────────────────────────────────────────────────
// from_wire — ToolCall
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a Gemini tool-call wire value (a `functionCall` part) into a canonical [`ToolCall`].
///
/// Because Gemini provides no call ID, this function generates one deterministically
/// via [`generate_gemini_id`]. Store the resulting `ToolCall::id` and echo it back
/// in `ToolResult::call_id` when calling [`denormalize_result`](crate::denormalize_result).
///
/// # Errors
/// - [`ToolNormalizeError::MissingField`] if `"functionCall"`, `"functionCall.name"`,
///   or `"functionCall.args"` are absent.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    // Gemini nests everything under a "functionCall" key inside the part object
    let function_call = raw
        .get("functionCall")
        .ok_or(ToolNormalizeError::MissingField {
            field: "functionCall",
            provider: PROVIDER,
        })?;

    // Extract the function name from inside the "functionCall" object
    let name = function_call
        .get("name")
        .and_then(Value::as_str)
        .ok_or(ToolNormalizeError::MissingField {
            field: "functionCall.name",
            provider: PROVIDER,
        })?
        .to_string();

    // Gemini's "args" is already a JSON object — no JSON-string parsing needed
    let arguments = function_call
        .get("args")
        .ok_or(ToolNormalizeError::MissingField {
            field: "functionCall.args",
            provider: PROVIDER,
        })?
        .clone();

    // Synthesize a deterministic ID since Gemini doesn't supply one
    let id = generate_gemini_id(&name, &arguments);

    Ok(ToolCall {
        id: ToolCallId(id),
        name,
        arguments,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolDefinition
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolDefinition`] into the Gemini wire format.
///
/// Produces a complete `function_declarations` wrapper ready to be inserted into
/// a Gemini `GenerateContent` request's `tools` array:
/// ```json
/// {
///   "function_declarations": [{
///     "name": "read_file",
///     "description": "...",
///     "parameters": { "type": "object", ... }
///   }]
/// }
/// ```
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    // Gemini wraps the declaration inside a "function_declarations" array.
    // The inner schema field is "parameters" (same name as JSON Schema, unlike Anthropic).
    Ok(json!({
        "function_declarations": [{
            "name": def.name,
            "description": def.description,
            "parameters": def.parameters,
        }]
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolResult
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolResult`] into the Gemini wire format.
///
/// Produces a `functionResponse` part for inclusion in a `role: "user"` content
/// array:
/// ```json
/// { "functionResponse": { "name": "read_file", "response": { "content": "..." } } }
/// ```
///
/// The `"response"` field must be a JSON object; text content is wrapped in
/// `{ "content": "..." }` and JSON content is wrapped in `{ "content": <value> }`.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    // Gemini's response field expects a JSON object — we standardize on { "content": ... }
    let response_content: Value = match &result.content {
        // Wrap plain text in an object so the response field is always an object
        ToolContent::Text(s) => json!({ "content": s }),
        // Wrap the JSON value directly — the model receives the structured object
        ToolContent::Json(v) => json!({ "content": v }),
    };

    Ok(json!({
        "functionResponse": {
            // Gemini identifies the result by function name (it has no ID to echo)
            "name": result.name,
            "response": response_content,
        }
    }))
}
