//! Error types for the memory_retrieve tool.

use thiserror::Error;

/// Errors that can occur during memory_retrieve tool execution.
#[derive(Debug, Error)]
pub enum MemoryRetrieveToolError {
    /// Failed to deserialize the JSON arguments into MemoryRetrieveArgs.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
