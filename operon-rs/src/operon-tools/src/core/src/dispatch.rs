//! Error types for the tool dispatcher.

use thiserror::Error;

/// Errors the dispatcher can return when handling a tool call.
#[derive(Debug, Error)]
pub enum ToolDispatchError {
    /// The model called a tool name that is not registered in this session.
    #[error("unknown tool: '{name}'")]
    UnknownTool { name: String },

    /// The model's arguments failed to deserialize into the tool's expected shape.
    /// The `reason` string is included in the error ToolResult sent back to the model,
    /// and the tool is marked degraded for the rest of the session.
    #[error("malformed arguments for tool '{tool}': {reason}")]
    MalformedArgs { tool: String, reason: String },

    /// The tool execution itself returned an unexpected internal error.
    /// Per-file or per-item failures are NOT this — those are embedded in ToolResult content.
    /// This variant is for bugs in the tool runtime (e.g. serialization failure).
    #[error("tool '{tool}' encountered an internal error: {reason}")]
    InternalError { tool: String, reason: String },
}
