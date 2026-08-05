//! Error types for the todo_delete tool.

use thiserror::Error;

/// Errors that can occur during todo_delete tool execution.
#[derive(Debug, Error)]
pub enum TodoDeleteToolError {
    /// Failed to deserialize the tool arguments JSON into TodoDeleteArgs.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
