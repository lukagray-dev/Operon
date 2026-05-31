//! Error types for the todo_update tool.

use thiserror::Error;

/// Errors that can occur during todo_update tool execution.
#[derive(Debug, Error)]
pub enum TodoUpdateToolError {
    /// Failed to deserialize the tool arguments JSON into TodoUpdateArgs.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
