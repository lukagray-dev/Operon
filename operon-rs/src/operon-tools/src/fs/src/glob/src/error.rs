//! Error types for the glob tool.
//!
//! Hey friend! This module defines error conditions that can occur during glob argument parsing.
use thiserror::Error;

/// Errors that can occur during glob tool execution.
#[derive(Debug, Error)]
pub enum GlobToolError {
    /// Top-level JSON arguments could not be deserialized.
    #[error("Failed to parse glob arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}

