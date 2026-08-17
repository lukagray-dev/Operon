//! Error types for the memory_edit tool.

use thiserror::Error;

/// Errors that can occur during memory_edit tool execution.
#[derive(Debug, Error)]
pub enum MemoryEditToolError {
    /// Failed to deserialize the JSON arguments into MemoryEditArgs.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
