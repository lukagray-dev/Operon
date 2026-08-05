//! NVIDIA NIM wire format normalization and denormalization.
//!
//! NIM exposes an OpenAI-compatible API. The tool-call wire format is structurally
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

/// The provider name used for error reporting and identification.
const PROVIDER: &str = "NVIDIA NIM";

/// Parse a NIM tool-call wire value into a canonical [`ToolCall`].
///
/// Since NVIDIA NIM uses the standard OpenAI tool call schema, we delegate
/// the parsing logic to OpenAI's helper. Any parse errors will refer to
/// "NVIDIA NIM".
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    openai::from_wire_tool_call_with_provider(raw, PROVIDER)
}

/// Serialize a [`ToolDefinition`] into the NIM wire format.
///
/// Delegates to the OpenAI tool definition serializer because NIM shares the
/// same tool definition format.
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_definition_with_provider(def, PROVIDER)
}

/// Serialize a [`ToolResult`] into the NIM wire format.
///
/// Delegates to the OpenAI tool result serializer because NIM shares the
/// same tool result structure.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_result_with_provider(result, PROVIDER)
}
