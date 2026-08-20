//! Cohere wire format normalization and denormalization.
//!
//! Cohere's tool-call shape is entirely distinct from OpenAI and Anthropic:
//! - Incoming calls are wrapped in a `"tool_call"` sub-object with `"parameters"`
//!   (already a JSON object, not a string).
//! - Tool results use a content-block array (`[{ "type": "text", "text": "..." }]`).
//! - Tool definitions use `"parameter_definitions"` instead of JSON Schema, with a
//!   Cohere-specific type system (`"str"`, `"int"`, `"float"`, `"bool"`, `"dict"`).
//!
//! ## Wire formats
//!
//! **Incoming tool call**:
//! ```json
//! {
//!   "id": "tool_call_id_1",
//!   "type": "tool_call",
//!   "tool_call": { "name": "read_file", "parameters": { "path": "/foo" } }
//! }
//! ```
//!
//! **Tool result** (`role: "tool"` message):
//! ```json
//! {
//!   "role": "tool",
//!   "tool_call_id": "tool_call_id_1",
//!   "content": [{ "type": "text", "text": "file contents here" }]
//! }
//! ```
//!
//! **Tool definition**:
//! ```json
//! {
//!   "name": "read_file",
//!   "description": "...",
//!   "parameter_definitions": {
//!     "path": { "description": "...", "type": "str", "required": true }
//!   }
//! }
//! ```
//!
//! ## Type mapping (JSON Schema → Cohere)
//!
//! | JSON Schema type | Cohere type |
//! |---|---|
//! | `"string"` | `"str"` |
//! | `"integer"` | `"int"` |
//! | `"number"` | `"float"` |
//! | `"boolean"` | `"bool"` |
//! | `"object"` | `"dict"` |
//! | anything else | `"str"` (safe fallback) |

use serde_json::{json, Value};

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult};

const PROVIDER: &str = "Cohere";

// ─────────────────────────────────────────────────────────────────────────────
// Schema type mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map a JSON Schema type string to Cohere's parameter type string.
///
/// Cohere does not use JSON Schema — it has its own flat type system.
/// Unmapped types (e.g. `"array"`, `"null"`) fall back to `"str"` rather than
/// failing hard, since Cohere still needs a type value to accept the definition.
fn json_schema_type_to_cohere(json_type: &str) -> &'static str {
    match json_type {
        "string" => "str",
        "integer" => "int",
        "number" => "float",
        "boolean" => "bool",
        "object" => "dict",
        // Arrays, nulls, unions, and unknown types all fall back to "str"
        _ => "str",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// from_wire — ToolCall
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a Cohere tool-call wire value into a canonical [`ToolCall`].
///
/// Expects the Cohere `"tool_call"` object format. The `"parameters"` field is
/// already a JSON object (Cohere does **not** JSON-encode it as a string).
///
/// # Errors
/// - [`ToolNormalizeError::MissingField`] if `"id"`, `"tool_call"`, `"tool_call.name"`,
///   or `"tool_call.parameters"` are absent.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    // Extract the top-level call ID, e.g. "tool_call_id_1"
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .ok_or(ToolNormalizeError::MissingField {
            field: "id",
            provider: PROVIDER,
        })?
        .to_string();

    // The actual call details are nested under the "tool_call" sub-object
    let tool_call = raw
        .get("tool_call")
        .ok_or(ToolNormalizeError::MissingField {
            field: "tool_call",
            provider: PROVIDER,
        })?;

    // Extract the function name from inside the sub-object
    let name = tool_call
        .get("name")
        .and_then(Value::as_str)
        .ok_or(ToolNormalizeError::MissingField {
            field: "tool_call.name",
            provider: PROVIDER,
        })?
        .to_string();

    // "parameters" is a JSON object already — no JSON-string decoding step needed
    let arguments = tool_call
        .get("parameters")
        .ok_or(ToolNormalizeError::MissingField {
            field: "tool_call.parameters",
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

/// Serialize a [`ToolDefinition`] into the Cohere wire format.
///
/// Converts the JSON Schema `parameters` object into Cohere's `parameter_definitions`
/// flat map. Only the top-level `"properties"` of the schema are converted;
/// deeply nested schemas are not supported by Cohere's format.
///
/// # Errors
/// - [`ToolNormalizeError::CohereSchemaConversion`] if `parameters` lacks a
///   top-level `"properties"` object.
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    // The "properties" key must exist at the top level of the JSON Schema
    let properties = def
        .parameters
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| ToolNormalizeError::CohereSchemaConversion {
            detail: "ToolDefinition.parameters must contain a top-level 'properties' \
                     object for Cohere conversion"
                .to_string(),
        })?;

    // The "required" array tells us which fields are mandatory (may be absent = none required)
    let required_fields: Vec<String> = def
        .parameters
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Build the Cohere parameter_definitions map: { "param_name": { type, description, required } }
    let mut param_defs = serde_json::Map::new();

    for (prop_name, prop_schema) in properties {
        // Use an empty description if the property schema omits it
        let description = prop_schema
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");

        // Convert the JSON Schema type string to Cohere's type system
        let json_type = if let Some(s) = prop_schema.get("type").and_then(Value::as_str) {
            s
        } else if let Some(arr) = prop_schema.get("type").and_then(Value::as_array) {
            arr.iter()
                .find_map(|v| v.as_str().filter(|&s| s != "null"))
                .unwrap_or("string")
        } else {
            "string"
        };

        let cohere_type = json_schema_type_to_cohere(json_type);

        // A field is required if its name appears in the schema's "required" array
        let is_required = required_fields.contains(prop_name);

        param_defs.insert(
            prop_name.clone(),
            json!({
                "description": description,
                "type": cohere_type,
                "required": is_required,
            }),
        );
    }

    Ok(json!({
        "name": def.name,
        "description": def.description,
        "parameter_definitions": Value::Object(param_defs),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// to_wire — ToolResult
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`ToolResult`] into the Cohere wire format.
///
/// Produces a `role: "tool"` message with a content-block array:
/// ```json
/// {
///   "role": "tool",
///   "tool_call_id": "...",
///   "content": [{ "type": "text", "text": "..." }]
/// }
/// ```
///
/// Both [`ToolContent::Text`] and [`ToolContent::Json`] are serialized to a text
/// block. JSON values are rendered as compact JSON strings inside the text field.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    // Cohere uses an array of typed content blocks (always text for tool results)
    let text: String = match &result.content {
        ToolContent::Text(s) => s.clone(),
        // Serialize JSON content compactly so Cohere can include it in the context
        ToolContent::Json(v) => v.to_string(),
    };

    Ok(json!({
        "role": "tool",
        "tool_call_id": result.call_id.0,
        // Cohere expects an array of content blocks, not a bare string
        "content": [{ "type": "text", "text": text }],
    }))
}
