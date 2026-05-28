//! DeepSeek wire format normalization and denormalization.
//!
//! DeepSeek's tool-call API is OpenAI-compatible. The wire formats are structurally
//! identical to OpenAI's, including the requirement that `"function.arguments"` is a
//! JSON-encoded string (not a JSON object). All logic is delegated to [`super::openai`].
//!
//! ## Notable DeepSeek differences (not wire-level)
//! - DeepSeek uses a different base URL and model namespace.
//! - DeepSeek may emit reasoning traces in the response (`"reasoning_content"` field).
//! - These are application-level concerns; the tool-call wire shape is unchanged.
//!
//! ## Wire formats
//!
//! Identical to OpenAI — see [`openai`](super::openai) for full documentation.

use serde_json::Value;

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolDefinition, ToolResult};

use super::openai;

const PROVIDER: &str = "DeepSeek";

/// Parse a DeepSeek tool-call wire value into a canonical [`ToolCall`].
///
/// Delegates to the OpenAI implementation. Error messages reference `"DeepSeek"`.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    openai::from_wire_tool_call_with_provider(raw, PROVIDER)
}

/// Serialize a [`ToolDefinition`] into the DeepSeek wire format.
///
/// Delegates to the OpenAI implementation — wire format is identical.
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_definition_with_provider(def, PROVIDER)
}

/// Serialize a [`ToolResult`] into the DeepSeek wire format.
///
/// Delegates to the OpenAI implementation — wire format is identical.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_result_with_provider(result, PROVIDER)
}
