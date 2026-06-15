//! Error types for the memory_retrieve tool.

use thiserror::Error;

/// Enumerates all possible errors that can occur while running the memory_retrieve tool.
#[derive(Debug, Error)]
pub enum MemoryRetrieveToolError {
    /// Failure during argument validation or parsing.
    #[error("failed to parse arguments: {0}")]
    ArgsParse(String),

    /// Failure interacting with the SQLite database.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
