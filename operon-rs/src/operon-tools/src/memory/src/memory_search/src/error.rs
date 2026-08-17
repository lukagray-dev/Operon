//! Error types for the memory_search tool.

use thiserror::Error;

/// Errors that can occur during memory_search tool execution.
#[derive(Debug, Error)]
pub enum MemorySearchToolError {
    /// Failed to deserialize the JSON arguments into MemorySearchArgs.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
