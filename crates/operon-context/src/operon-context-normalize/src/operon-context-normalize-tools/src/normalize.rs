//! Public entry-point functions for tool-call normalization and denormalization.
//!
//! This module provides three functions that form the entire public API surface
//! for converting between provider wire formats and canonical types:
//!
//! | Function | Direction | Input | Output |
//! |---|---|---|---|
//! | [`normalize`] | wire → canonical | raw provider JSON | [`ToolCall`] |
//! | [`denormalize_definition`] | canonical → wire | [`ToolDefinition`] | provider JSON |
//! | [`denormalize_result`] | canonical → wire | [`ToolResult`] | provider JSON |
//!
//! All three functions are thin wrappers over the [`FromWire`] / [`ToWire`] trait
//! dispatch in [`crate::provider`]. The wrapper layer exists to provide a stable,
//! function-based API that callers can use without importing any traits.

use serde_json::Value;

use crate::error::ToolNormalizeError;
use crate::provider::{FromWire, Provider, ToWire};
use crate::types::{ToolCall, ToolDefinition, ToolResult};

/// Convert a raw provider tool-call JSON blob into a canonical [`ToolCall`].
///
/// This is the primary entry point for the normalization direction. Pass the
/// exact JSON value the provider returned (the individual tool-call object, not
/// the entire response envelope) along with the [`Provider`] that produced it.
///
/// # Arguments
/// - `raw` — the provider-specific tool-call JSON object.
/// - `provider` — which provider's wire format to use for parsing.
///
/// # Errors
/// - [`ToolNormalizeError::MissingField`] — a required key is absent.
/// - [`ToolNormalizeError::ArgumentParseFailed`] — OpenAI-style `arguments` string is not valid JSON.
/// - [`ToolNormalizeError::UnknownShape`] — OpenRouter could not detect the underlying shape.
///
/// # Example
/// ```
/// use operon_context_normalize_tools::{normalize, Provider};
/// use serde_json::json;
///
/// let wire = json!({
///     "id": "call_abc",
///     "type": "function",
///     "function": { "name": "read_file", "arguments": "{\"path\":\"/foo\"}" }
/// });
/// let call = normalize(wire, &Provider::OpenAI).unwrap();
/// assert_eq!(call.name, "read_file");
/// assert_eq!(call.arguments["path"], "/foo");
/// ```
pub fn normalize(raw: Value, provider: &Provider) -> Result<ToolCall, ToolNormalizeError> {
    // Delegate to the FromWire trait implementation on ToolCall, which dispatches
    // based on the Provider variant to the appropriate provider module.
    ToolCall::from_wire(raw, provider)
}

/// Convert a [`ToolDefinition`] into the provider-specific schema format.
///
/// Use this function to produce the JSON that goes into the `"tools"` (or equivalent)
/// field of a provider API request. Each provider has its own schema format:
/// Anthropic uses `"input_schema"`, Gemini wraps in `"function_declarations"`,
/// Cohere converts JSON Schema to `"parameter_definitions"`, etc.
///
/// # Arguments
/// - `def` — the canonical tool definition to serialize.
/// - `provider` — which provider's wire format to produce.
///
/// # Errors
/// - [`ToolNormalizeError::CohereSchemaConversion`] — only for `Provider::Cohere`, when
///   `def.parameters` lacks a `"properties"` key.
/// - [`ToolNormalizeError::SerializeFailed`] — in the extremely unlikely case that
///   `serde_json` cannot serialize the in-memory value.
///
/// # Example
/// ```
/// use operon_context_normalize_tools::{denormalize_definition, Provider, ToolDefinition};
/// use serde_json::json;
///
/// let def = ToolDefinition {
///     name: "read_file".to_string(),
///     description: "Read a file.".to_string(),
///     parameters: json!({ "type": "object", "properties": { "path": { "type": "string" } } }),
/// };
/// let wire = denormalize_definition(&def, &Provider::Anthropic).unwrap();
/// // Anthropic uses "input_schema" instead of "parameters"
/// assert!(wire.get("input_schema").is_some());
/// ```
pub fn denormalize_definition(
    def: &ToolDefinition,
    provider: &Provider,
) -> Result<Value, ToolNormalizeError> {
    // Delegate to the ToWire trait implementation on ToolDefinition
    def.to_wire(provider)
}

/// Convert a [`ToolResult`] into the provider-specific message format to insert
/// into conversation context.
///
/// Use this function to produce the JSON message that gets appended to the
/// conversation history after your code has executed a tool call. Each provider
/// expects a different message structure for tool results.
///
/// # Arguments
/// - `result` — the canonical tool result to serialize.
/// - `provider` — which provider's wire format to produce.
///
/// # Errors
/// - [`ToolNormalizeError::SerializeFailed`] — in the extremely unlikely case that
///   `serde_json` cannot serialize the in-memory value.
///
/// # Example
/// ```
/// use operon_context_normalize_tools::{
///     denormalize_result, Provider, ToolCallId, ToolContent, ToolResult
/// };
///
/// let result = ToolResult {
///     call_id: ToolCallId("call_abc".to_string()),
///     name: "read_file".to_string(),
///     content: ToolContent::Text("file contents".to_string()),
///     is_error: false,
/// };
/// let wire = denormalize_result(&result, &Provider::OpenAI).unwrap();
/// assert_eq!(wire["role"], "tool");
/// assert_eq!(wire["tool_call_id"], "call_abc");
/// ```
pub fn denormalize_result(
    result: &ToolResult,
    provider: &Provider,
) -> Result<Value, ToolNormalizeError> {
    // Delegate to the ToWire trait implementation on ToolResult
    result.to_wire(provider)
}
