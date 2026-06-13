//! Canonical tool-call and tool-result types used throughout Operon.
//!
//! Under the plain-text tag protocol, these types serve as the internal, stable
//! representation of tool invocations and results, without any provider-specific
//! JSON schemas or wire formats.

pub use operon_context_normalize_messages::tools::{
    ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult,
};
