//! Mistral wire format normalization and denormalization.
//!
//! Mistral's API is OpenAI-compatible for tool calls. The wire format including
//! JSON-encoded string arguments is identical to OpenAI's. All logic is delegated
//! to [`super::openai`].
//!
//! ## Notable Mistral differences (not wire-level)
//! - Mistral uses a different base URL (`api.mistral.ai`).
//! - Tool choice options differ slightly in naming from OpenAI (`"any"` vs `"required"`).
//! - These are request-level concerns; the tool-call response wire shape is unchanged.
//!
//! ## Wire formats
//!
//! Identical to OpenAI — see [`openai`](super::openai) for full documentation.

use serde_json::Value;

use crate::error::ToolNormalizeError;
use crate::types::{ToolCall, ToolDefinition, ToolResult};

use super::openai;

const PROVIDER: &str = "Mistral";

/// Parse a Mistral tool-call wire value into a canonical [`ToolCall`].
///
/// Delegates to the OpenAI implementation. Error messages reference `"Mistral"`.
pub fn from_wire_tool_call(raw: Value) -> Result<ToolCall, ToolNormalizeError> {
    openai::from_wire_tool_call_with_provider(raw, PROVIDER)
}

/// Serialize a [`ToolDefinition`] into the Mistral wire format.
///
/// Delegates to the OpenAI implementation — wire format is identical.
pub fn to_wire_tool_definition(def: &ToolDefinition) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_definition_with_provider(def, PROVIDER)
}

/// Serialize a [`ToolResult`] into the Mistral wire format.
///
/// Delegates to the OpenAI implementation — wire format is identical.
pub fn to_wire_tool_result(result: &ToolResult) -> Result<Value, ToolNormalizeError> {
    openai::to_wire_tool_result_with_provider(result, PROVIDER)
}
