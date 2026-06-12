//! Error types for the todo_list tool.

use thiserror::Error;

/// Errors that can occur during todo_list tool execution.
#[derive(Debug, Error)]
pub enum TodoListToolError {
    /// Failed to parse the tool arguments from the plain-text attr map.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
