//! Error types for the delete tool.
//!
//! This module defines all error conditions that can occur during delete tool
//! argument parsing. Per-file deletion failures are NOT represented here — they are
//! returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during delete tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual file deletion failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum DeleteToolError {
    /// Failed to deserialize the tool arguments JSON into DeleteArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the DeleteArgs schema (e.g., missing "path" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
