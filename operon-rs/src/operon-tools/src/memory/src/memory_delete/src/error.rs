//! Error types for the memory_delete tool.

use thiserror::Error;

/// Errors that can occur during memory_delete tool execution.
#[derive(Debug, Error)]
pub enum MemoryDeleteToolError {
    /// Failed to deserialize the JSON arguments into MemoryDeleteArgs.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
