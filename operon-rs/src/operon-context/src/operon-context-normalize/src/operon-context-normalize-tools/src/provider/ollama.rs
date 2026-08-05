//! Ollama wire format normalization and denormalization.
//!
//! Ollama's tool-call wire format is **identical to OpenAI's**. This module is a
//! thin delegation layer that exists purely for semantic clarity and correct provider
//! names in error messages. All logic lives in [`super::openai`].
//!
//! ## Wire formats
//!
//! Identical to OpenAI — see [`openai`](super::openai) for full documentation.
//! - Incoming tool call: `"function.arguments"` is a JSON-encoded string.
//! - Tool result: `role: "tool"` message with a string `"content"` field.
//! - Tool definition: `{ "type": "function", "function": { ... } }` envelope.

use serde_json::Value;

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolDefinition, ToolResult};

use super::openai;

// Provider name string used in all error messages for this module
const PROVIDER: &str = "Ollama";

/// Parse an Ollama tool-call wire value into a canonical [`ToolCall`].
///
/// Delegates entirely to the OpenAI implementation — the wire formats are identical.
/// Error messages will reference `"Ollama"` rather than `"OpenAI"`.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    openai::from_wire_tool_call_with_provider(raw, PROVIDER)
}

/// Serialize a [`ToolDefinition`] into the Ollama wire format.
///
/// Delegates entirely to the OpenAI implementation — the wire formats are identical.
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_definition_with_provider(def, PROVIDER)
}

/// Serialize a [`ToolResult`] into the Ollama wire format.
///
/// Delegates entirely to the OpenAI implementation — the wire formats are identical.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_result_with_provider(result, PROVIDER)
}
