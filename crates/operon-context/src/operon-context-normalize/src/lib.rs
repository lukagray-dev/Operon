//! Facade crate for Operon context normalization.
//!
//! This crate re-exports all normalization sub-crates as a unified API surface.
//! Each sub-crate is also usable independently when only one normalization
//! domain is needed.
//!
//! Re-exported modules:
//! - [`tools`]: canonical tool-call types and provider wire normalization.
//! - [`reasoning`]: canonical reasoning/thinking block normalization.
//! - [`messages`]: canonical conversation message normalization.
//! - [`stream`]: canonical stream-event parsing and stream assembly.

pub use operon_context_normalize_messages as messages;
pub use operon_context_normalize_reasoning as reasoning;
pub use operon_context_normalize_stream as stream;
pub use operon_context_normalize_tools as tools;

pub use operon_context_normalize_tools::{
    Provider, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult,
};
pub use operon_context_normalize_reasoning::{ReasoningBlock, ReasoningSignature};
pub use operon_context_normalize_messages::{ContentBlock, ConversationMessage, MessageRole, StopReason};
pub use operon_context_normalize_stream::{AssemblerOutput, StreamEvent};
