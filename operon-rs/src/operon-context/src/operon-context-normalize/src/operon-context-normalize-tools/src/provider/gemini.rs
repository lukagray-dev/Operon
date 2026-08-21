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

/// Sanitizes and projects a standard JSON Schema into Gemini's OpenAPI-compatible dialect.
///
/// Hey friend! Google Gemini's tool definition parser uses strict protobuf schemas.
/// Specifically:
/// 1. `type` MUST be a single scalar string (e.g. `"string"` or `"integer"`). If standard JSON Schema
///    uses an array like `["string", "null"]` for optional fields, Gemini throws:
///    `Proto field is not repeating, cannot start list.`
///    We convert `type: ["string", "null"]` into `type: "string", nullable: true`.
/// 2. `properties` and `required` on non-object types are cleaned up.
/// 3. Unsupported schema keys like `$schema`, `additionalProperties` (if false) are omitted.
/// 4. Enum items on string properties are stringified.
pub fn sanitize_gemini_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();

            for (key, val) in map {
                // Skip unsupported meta keys that Gemini protobuf schema rejects
                if key == "$schema" || key == "$id" || key == "additionalProperties" {
                    continue;
                }

                if key == "type" {
                    if let Some(arr) = val.as_array() {
                        // Extract the primary non-null type (e.g. ["string", "null"] -> "string")
                        let non_null_type = arr
                            .iter()
                            .find_map(|t| t.as_str().filter(|&s| s != "null"))
                            .unwrap_or("string");
                        let is_nullable = arr.iter().any(|t| t.as_str() == Some("null"));

                        out.insert("type".to_string(), Value::String(non_null_type.to_string()));
                        if is_nullable {
                            out.insert("nullable".to_string(), Value::Bool(true));
                        }
                    } else if let Some(s) = val.as_str() {
                        out.insert("type".to_string(), Value::String(s.to_string()));
                    }
                } else if key == "properties" {
                    if let Some(props) = val.as_object() {
                        let mut sanitized_props = serde_json::Map::new();
                        for (prop_name, prop_val) in props {
                            sanitized_props
                                .insert(prop_name.clone(), sanitize_gemini_schema(prop_val));
                        }
                        out.insert("properties".to_string(), Value::Object(sanitized_props));
                    }
                } else if key == "items" {
                    out.insert("items".to_string(), sanitize_gemini_schema(val));
                } else if key == "anyOf" || key == "oneOf" || key == "allOf" {
                    if let Some(arr) = val.as_array() {
                        let sanitized_arr: Vec<Value> =
                            arr.iter().map(sanitize_gemini_schema).collect();
                        out.insert(key.clone(), Value::Array(sanitized_arr));
                    }
                } else if key == "enum" {
                    if let Some(arr) = val.as_array() {
                        let str_arr: Vec<Value> = arr
                            .iter()
                            .map(|v| match v {
                                Value::String(s) => Value::String(s.clone()),
                                other => Value::String(other.to_string()),
                            })
                            .collect();
                        out.insert("enum".to_string(), Value::Array(str_arr));
                    }
                } else if key == "required" {
                    if let Some(reqs) = val.as_array() {
                        out.insert("required".to_string(), Value::Array(reqs.clone()));
                    }
                } else {
                    out.insert(key.clone(), val.clone());
                }
            }

            // Ensure object schemas have a default type = "object" if properties are present
            if out.contains_key("properties") && !out.contains_key("type") {
                out.insert("type".to_string(), Value::String("object".to_string()));
            }

            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sanitize_gemini_schema).collect()),
        other => other.clone(),
    }
}

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
    let sanitized_params = sanitize_gemini_schema(&def.parameters);

    Ok(json!({
        "function_declarations": [{
            "name": def.name,
            "description": def.description,
            "parameters": sanitized_params,
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
    // Gemini's response field expects a JSON object — we standardize on { "name": ..., "content": ... }
    let response_content: Value = match &result.content {
        // Wrap plain text in an object with name and content
        ToolContent::Text(s) => json!({ "name": result.name, "content": s }),
        // Wrap the JSON value directly with name and content
        ToolContent::Json(v) => json!({ "name": result.name, "content": v }),
    };

    Ok(json!({
        "functionResponse": {
            // Gemini identifies the result by function name (it has no ID to echo)
            "name": result.name,
            "response": response_content,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_array_types_for_gemini() {
        let input_schema = json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": ["string", "null"],
                    "description": "Path to file"
                },
                "lines": {
                    "type": ["integer", "null"]
                }
            },
            "required": ["path"]
        });

        let sanitized = sanitize_gemini_schema(&input_schema);
        assert_eq!(sanitized["properties"]["path"]["type"], "string");
        assert_eq!(sanitized["properties"]["path"]["nullable"], true);
        assert_eq!(sanitized["properties"]["lines"]["type"], "integer");
        assert_eq!(sanitized["properties"]["lines"]["nullable"], true);
    }
}
