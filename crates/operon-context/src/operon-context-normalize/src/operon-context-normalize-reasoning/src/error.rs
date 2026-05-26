//! Error types for the `operon-context-normalize-reasoning` crate.
//!
//! All normalization and denormalization operations return a single
//! [`ReasoningNormalizeError`] on failure. Callers only ever need to import
//! one type.
//!
//! # Example
//! ```
//! use operon_context_normalize_reasoning::{ReasoningNormalizeError, Provider};
//! use operon_context_normalize_reasoning::normalize_reasoning;
//! use serde_json::json;
//!
//! let err = normalize_reasoning(json!(null), &Provider::DeepSeek).unwrap_err();
//! assert!(matches!(
//!     err,
//!     ReasoningNormalizeError::MissingField { field: "reasoning_content", .. }
//! ));
//! ```

use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Error enum
// ─────────────────────────────────────────────────────────────────────────────

/// All errors that can occur when normalizing or denormalizing provider
/// reasoning/thinking payloads.
///
/// Each variant corresponds to a distinct failure mode. The `provider` field
/// on most variants identifies which provider's wire format was being
/// processed, enabling the caller to produce useful diagnostics.
#[derive(Debug, Error)]
pub enum ReasoningNormalizeError {
    /// A required JSON field was absent or had the wrong type in the
    /// provider's wire format.
    ///
    /// - `field`: the JSON key that was expected, e.g. `"thinking"`.
    /// - `provider`: which provider's format was being parsed, e.g.
    ///   `"Anthropic"`.
    #[error("missing required field `{field}` in {provider} reasoning wire format")]
    MissingField {
        /// The JSON key that was absent or had an unexpected type.
        field: &'static str,
        /// The provider whose wire format was being parsed.
        provider: &'static str,
    },

    /// The provider does not expose reasoning content via its public API.
    ///
    /// Groq, Mistral, and Cohere all fall into this category. Neither
    /// `normalize_reasoning` nor `denormalize_reasoning` can succeed for
    /// these providers.
    #[error("provider {provider} does not expose reasoning content via its API")]
    NotSupported {
        /// The provider that does not support reasoning content.
        provider: &'static str,
    },

    /// Serializing a `ReasoningBlock` back to the provider wire format failed.
    ///
    /// This is rare in practice because most serialization uses the
    /// `serde_json::json!` macro, which cannot fail. It can theoretically
    /// occur for values that exceed serde_json's depth or size limits.
    #[error("failed to serialize reasoning block for provider {provider}: {source}")]
    SerializeFailed {
        /// The provider whose serialization failed.
        provider: &'static str,
        /// The underlying serde_json error.
        source: serde_json::Error,
    },

    /// The reasoning array returned by the provider was empty or contained no
    /// usable text blocks.
    ///
    /// Currently applies only to OpenAI's `reasoning_summary` array — an
    /// empty array means there is no reasoning content to normalize.
    #[error("{provider} reasoning array was empty or contained no usable text blocks")]
    EmptyReasoningSummary {
        /// The provider that returned an empty reasoning summary.
        provider: &'static str,
    },

    /// The input value did not match any known reasoning wire shape for this
    /// provider.
    ///
    /// Currently used by OpenRouter's key-based shape-detection logic when the
    /// incoming value has none of the expected keys. The `detail` field
    /// explains what was found versus what was expected.
    #[error("unknown or unsupported reasoning shape for provider {provider}: {detail}")]
    UnknownShape {
        /// The provider for which shape detection failed.
        provider: &'static str,
        /// A human-readable description of why the shape was rejected.
        detail: String,
    },
}
