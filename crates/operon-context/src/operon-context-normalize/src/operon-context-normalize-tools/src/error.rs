//! Error types for the `operon-context-normalize-tools` crate.
//!
//! All errors produced during tool-call normalization or denormalization flow through
//! [`ToolNormalizeError`]. Each variant carries enough structured context (provider name,
//! field name, source error) to let callers pinpoint *exactly* which provider and which
//! field triggered the problem — critical when debugging mismatched wire formats across
//! ten different LLM providers.
//!
//! # Design rationale
//! Using `thiserror` keeps the `Display` implementations in sync with the struct fields
//! automatically, and lets callers match on variants without converting to strings.

use thiserror::Error;

/// The single error type returned by every public function in this crate.
///
/// Constructed internally by provider-specific normalization code; callers should
/// match on variants to decide whether the failure is recoverable (e.g., a bad
/// argument JSON string from a partially-streamed response) or fatal (e.g., a
/// completely unrecognised wire shape from an unknown provider).
///
/// # Example
/// ```
/// use operon_context_normalize_tools::{normalize, Provider, ToolNormalizeError};
/// use serde_json::json;
///
/// // Missing the required "id" field → MissingField error
/// let raw = json!({ "name": "my_tool", "input": {} });
/// let err = normalize(raw, &Provider::Anthropic).unwrap_err();
/// assert!(matches!(err, ToolNormalizeError::MissingField { field: "id", .. }));
/// ```
#[derive(Debug, Error)]
pub enum ToolNormalizeError {
    /// A required JSON field was absent from the provider's wire payload.
    ///
    /// This usually means the provider returned a malformed, truncated, or
    /// otherwise unexpected response — for example when streaming is interrupted
    /// mid-message.
    #[error("missing required field `{field}` in {provider} tool-call wire format")]
    MissingField {
        /// The dot-notation name of the missing field, e.g. `"id"` or `"function.name"`.
        field: &'static str,
        /// Human-readable provider name, e.g. `"OpenAI"`, `"Anthropic"`.
        provider: &'static str,
    },

    /// The `arguments` (or equivalent) field contained a string that is not valid JSON.
    ///
    /// OpenAI-compatible providers encode tool arguments as a JSON-encoded *string*
    /// (i.e., the value is `"{\"path\":\"/foo\"}"` rather than `{"path":"/foo"}`).
    /// If the model returns a truncated or hallucinated argument string, this error is returned.
    #[error("failed to parse tool arguments as JSON for provider {provider}: {source}")]
    ArgumentParseFailed {
        /// The provider whose argument field failed to parse.
        provider: &'static str,
        /// The underlying `serde_json` deserialization error, which includes the position
        /// in the string where parsing failed.
        source: serde_json::Error,
    },

    /// The raw JSON does not match any known wire shape for the given provider.
    ///
    /// Currently only returned by the OpenRouter provider, which must detect whether
    /// the underlying shape is OpenAI-style or Anthropic-style. If neither shape is
    /// detected from the key set, this error is returned.
    #[error("unknown or unsupported tool-call shape for provider {provider}: {detail}")]
    UnknownShape {
        /// The provider that could not recognise the shape.
        provider: &'static str,
        /// A human-readable description of what was found instead of the expected shape.
        detail: String,
    },

    /// Serializing a [`ToolDefinition`](crate::ToolDefinition) into the provider wire
    /// format failed.
    ///
    /// In practice this should be extremely rare — the input is already well-typed
    /// in-memory Rust data. The error can occur when a `serde_json::Value` inside
    /// `ToolDefinition::parameters` contains a non-string map key, which is invalid JSON.
    #[error("failed to serialize tool definition for provider {provider}: {source}")]
    SerializeFailed {
        /// The provider for which serialization was attempted.
        provider: &'static str,
        /// The underlying `serde_json` serialization error.
        source: serde_json::Error,
    },

    /// Converting a JSON Schema `parameters` object to Cohere's `parameter_definitions`
    /// format (or vice-versa) encountered an unexpected or unsupported structure.
    ///
    /// Cohere uses a flat `parameter_definitions` map rather than a JSON Schema, so
    /// the conversion can fail if the input schema is missing the `"properties"` key
    /// or uses deeply nested structures that have no Cohere equivalent.
    #[error("Cohere parameter_definitions conversion failed: {detail}")]
    CohereSchemaConversion {
        /// A human-readable description of why the conversion failed.
        detail: String,
    },
}
