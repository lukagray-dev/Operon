//! Error types for the load_tools tool.

/// Errors that can occur when executing load_tools.
#[derive(Debug, thiserror::Error)]
pub enum LoadToolsError {
    /// Failed to deserialize tool arguments from JSON.
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
