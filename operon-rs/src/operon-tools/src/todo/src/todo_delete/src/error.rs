//! Error types for the todo_delete tool.

use thiserror::Error;

/// Errors that can occur during todo_delete tool execution.
#[derive(Debug, Error)]
pub enum TodoDeleteToolError {
    /// Failed to parse the tool arguments from the plain-text attr map.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
