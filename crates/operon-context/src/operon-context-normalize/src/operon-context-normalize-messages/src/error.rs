//! Error types for the `operon-context-normalize-messages` crate.
//!
//! All message normalization and denormalization failures flow through
//! [`MessageNormalizeError`]. Each variant carries structured context
//! (`provider`, `field`, `detail`, `source`) so callers can log precise
//! diagnostics when provider wire payloads drift.

use thiserror::Error;

/// All errors that can occur while converting provider wire message JSON to and
/// from canonical [`ConversationMessage`](crate::ConversationMessage) values.
#[derive(Debug, Error)]
pub enum MessageNormalizeError {
    /// A required JSON field was absent from the provider payload.
    #[error("missing required field `{field}` in {provider} message wire format")]
    MissingField {
        /// The missing field name (often dot-notation), e.g. `"role"` or
        /// `"choices[0].message"`.
        field: &'static str,
        /// Human-readable provider label, e.g. `"OpenAI"` or `"Anthropic"`.
        provider: &'static str,
    },

    /// The provider returned a role string this crate does not recognize.
    #[error("unknown role `{role}` in {provider} message wire format")]
    UnknownRole {
        /// Raw role value from the provider payload.
        role: String,
        /// Human-readable provider label.
        provider: &'static str,
    },

    /// The payload shape did not match any supported wire format for the
    /// provider.
    #[error("unknown or unsupported message shape for provider {provider}: {detail}")]
    UnknownShape {
        /// Provider label where detection failed.
        provider: &'static str,
        /// Human-readable explanation of what keys/shape were observed.
        detail: String,
    },

    /// Serializing an in-memory canonical message to provider wire JSON failed.
    #[error("failed to serialize message for provider {provider}: {source}")]
    SerializeFailed {
        /// Provider label where serialization failed.
        provider: &'static str,
        /// Underlying serde_json error.
        source: serde_json::Error,
    },

    /// A content block exists in canonical form but the target provider does not
    /// support that block type in messages.
    #[error("unsupported content type for provider {provider}: {detail}")]
    UnsupportedContentType {
        /// Provider label where block conversion failed.
        provider: &'static str,
        /// Human-readable description of unsupported content.
        detail: String,
    },
}

/// Crate-local convenience result alias.
pub type Result<T> = std::result::Result<T, MessageNormalizeError>;
