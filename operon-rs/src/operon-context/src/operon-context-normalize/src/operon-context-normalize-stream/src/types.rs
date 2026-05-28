//! Canonical streaming types for `operon-context-normalize-stream`.

use operon_context_normalize_messages::StopReason;
use operon_context_normalize_tools::ToolCall;
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

    /// The first chunk for a tool call index.
    ToolCallStart {
        index: usize,
        id: Option<String>,
        name: Option<String>,
    },

    /// A tool-call arguments fragment for a specific index.
    ToolCallDelta {
        index: usize,
        arguments_fragment: String,
    },

    /// Explicit end marker for a tool call index.
    ToolCallEnd { index: usize },

    /// Full one-shot tool call with parsed arguments.
    ToolCallComplete {
        index: usize,
        id: Option<String>,
        name: String,
        arguments: serde_json::Value,
    },

    /// Raw provider stop reason.
    StopReason { raw: String },

    /// Raw provider usage metadata payload.
    UsageMeta { raw: serde_json::Value },

    /// Stream-start metadata (when present).
    StreamStart { model: Option<String> },

    /// Provider ping/keepalive.
    Ping,
}

/// Internal per-index buffer for fragmented tool calls.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToolCallBuffer {
    /// Provider tool index emitted in the stream.
    pub index: usize,
    /// Optional tool call ID, if the provider emitted it.
    pub id: Option<String>,
    /// Optional function name, if the provider emitted it.
    pub name: Option<String>,
    /// Concatenated raw JSON-encoded arguments fragments.
    pub arguments_json: String,
    /// Whether an explicit end marker was seen.
    pub complete: bool,
}

/// Output item emitted by `StreamAssembler`.
#[derive(Debug, Clone, PartialEq)]
pub enum AssemblerOutput {
    /// A complete text segment ready for immediate rendering.
    Text(String),

    /// A complete reasoning block emitted on finish/drain boundaries.
    Reasoning {
        /// Reasoning text content.
        text: String,
        /// Optional reasoning signature carried by some providers.
        signature: Option<String>,
    },

    /// A complete tool call with fully parsed arguments.
    ToolCall(ToolCall),

    /// The stream ended and stop reason has been normalized.
    StreamEnded {
        /// Canonical stop reason, when one was observed.
        stop_reason: Option<StopReason>,
    },

    /// No externally visible output yet; state was buffered.
    Pending,
}
