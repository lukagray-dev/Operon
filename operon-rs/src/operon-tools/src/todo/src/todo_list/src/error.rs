//! Error types for the todo_list tool.

use thiserror::Error;

/// Errors that can occur during todo_list tool execution.
#[derive(Debug, Error)]
pub enum TodoListToolError {
    /// Failed to deserialize the tool arguments JSON into TodoListArgs.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
