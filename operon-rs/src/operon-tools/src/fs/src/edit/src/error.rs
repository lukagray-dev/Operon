//! Error types for the edit tool.
//!
//! Hey friend! This module defines all error conditions that can occur during edit tool
//! argument parsing and execution. Top-level deserialization errors return `Err(EditToolError)`.
//! Per-hunk edit failures are returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during edit tool argument deserialization.
#[derive(Debug, Error)]
pub enum EditToolError {
    /// Failed to deserialize tool arguments JSON into EditArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the EditArgs schema (e.g., missing "path" or "patch" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
