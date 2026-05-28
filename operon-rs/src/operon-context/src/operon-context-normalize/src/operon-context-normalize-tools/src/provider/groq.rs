//! Groq wire format normalization and denormalization.
//!
//! Groq exposes an OpenAI-compatible API. The tool-call wire format is structurally
//! identical to OpenAI's, including JSON-encoded string arguments. All logic is
//! delegated to [`super::openai`].
//!
//! ## Wire formats
//!
//! Identical to OpenAI — see [`openai`](super::openai) for full documentation.

use serde_json::Value;

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolDefinition, ToolResult};

use super::openai;

const PROVIDER: &str = "Groq";

/// Parse a Groq tool-call wire value into a canonical [`ToolCall`].
///
/// Delegates to the OpenAI implementation. Error messages reference `"Groq"`.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    openai::from_wire_tool_call_with_provider(raw, PROVIDER)
}

/// Serialize a [`ToolDefinition`] into the Groq wire format.
///
/// Delegates to the OpenAI implementation — wire format is identical.
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_definition_with_provider(def, PROVIDER)
}

/// Serialize a [`ToolResult`] into the Groq wire format.
///
/// Delegates to the OpenAI implementation — wire format is identical.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_result_with_provider(result, PROVIDER)
}
