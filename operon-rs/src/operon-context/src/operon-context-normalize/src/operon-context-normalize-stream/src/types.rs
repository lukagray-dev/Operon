//! Canonical streaming types for `operon-context-normalize-stream`.

use operon_context_normalize_messages::StopReason;
use serde::{Deserialize, Serialize};

/// Canonical streaming event emitted by `parse_line`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StreamEvent {
    /// A text delta fragment.
    TextDelta { text: String },

    /// A reasoning/thinking delta fragment.
    ReasoningDelta { text: String },

    /// A reasoning block signature delta (provider-specific opaque value).
    ReasoningSignature { signature: String },

    /// Raw provider stop reason.
    StopReason { raw: String },

    /// Raw provider usage metadata payload.
    UsageMeta { raw: serde_json::Value },

    /// Stream-start metadata (when present).
    StreamStart { model: Option<String> },

    /// Provider ping/keepalive.
    Ping,
}

/// Output item emitted by `StreamAssembler`.
#[derive(Debug, Clone, PartialEq)]
pub enum AssemblerOutput {
    /// A complete text segment ready for immediate rendering.
    Text(String),

    /// A reasoning/thinking delta fragment.
    ReasoningDelta(String),

    /// A complete reasoning block emitted on finish/drain boundaries.
    Reasoning {
        /// Reasoning text content.
        text: String,
        /// Optional reasoning signature carried by some providers.
        signature: Option<String>,
    },

    /// The stream ended and stop reason has been normalized.
    StreamEnded {
        /// Canonical stop reason, when one was observed.
        stop_reason: Option<StopReason>,
    },

    /// No externally visible output yet; state was buffered.
    Pending,
}
