//! Error types for the memory_add tool.
//!
//! Top-level errors that prevent the tool from running at all (parse failures).
//! Validation errors (empty content) are returned as ToolResult is_error=true.

use thiserror::Error;

/// Errors that can occur during memory_add tool execution.
#[derive(Debug, Error)]
pub enum MemoryAddToolError {
    /// Failed to deserialize the JSON arguments into MemoryAddArgs.
    ///
    /// This occurs when required fields like "content" are missing or the JSON
    /// shape doesn't match — the dispatcher will mark the tool as degraded and
    /// return the detailed description to the model on the next turn.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
