//! Error types for the `operon-context-normalize-stream` crate.

use thiserror::Error;

/// All errors that can occur while parsing streaming wire payloads or
/// assembling canonical stream output.
#[derive(Debug, Error)]
pub enum StreamNormalizeError {
    /// The line was expected to contain JSON but failed deserialization.
    #[error("malformed JSON in {provider} stream payload: {source}")]
    MalformedJson {
        /// Provider label used in diagnostics.
        provider: &'static str,
        /// Underlying JSON parse error.
        source: serde_json::Error,
    },

    /// A required field was missing from the line payload.
    #[error("missing required field `{field}` in {provider} stream wire format")]
    MissingField {
        /// Field name (often dot-notation) that was required.
        field: &'static str,
        /// Provider label used in diagnostics.
        provider: &'static str,
    },

    /// A provider event type was recognized as JSON but not as a supported
    /// stream event.
    #[error("unknown stream event type `{event_type}` in {provider} wire format")]
    UnknownEventType {
        /// Raw provider event type or a key-based description.
        event_type: String,
        /// Provider label used in diagnostics.
        provider: &'static str,
    },

    /// Assembled tool arguments could not be parsed as valid JSON.
    #[error("failed to parse assembled tool arguments for {provider} tool index {index}: {source}")]
    ToolArgsParseFailed {
        /// Provider label used in diagnostics.
        provider: &'static str,
        /// Tool call index in the stream.
        index: usize,
        /// Underlying JSON parse error.
        source: serde_json::Error,
    },

    /// The caller ended the stream while assembler state was incomplete.
    #[error("incomplete stream state for {provider}: {detail}")]
    AssemblerIncomplete {
        /// Provider label used in diagnostics.
        provider: &'static str,
        /// Human-readable detail about what remained incomplete.
        detail: String,
    },
}

/// Crate-local result alias.
pub type Result<T> = std::result::Result<T, StreamNormalizeError>;
