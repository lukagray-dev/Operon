//! Error types for the write tool.
//!
//! This module defines all error conditions that can occur during write tool
//! argument parsing. Per-file write failures are NOT represented here — they are
//! returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during write tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual file write failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum WriteToolError {
    /// Failed to deserialize the tool arguments JSON into WriteArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the WriteArgs schema (e.g., missing "path" or "content" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
