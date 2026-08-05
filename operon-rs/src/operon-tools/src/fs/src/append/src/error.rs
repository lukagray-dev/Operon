//! Error types for the append tool.
//!
//! This module defines all error conditions that can occur during append tool
//! argument parsing. Per-file append failures are NOT represented here — they are
//! returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during append tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual file append failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum AppendToolError {
    /// Failed to deserialize the tool arguments JSON into AppendArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the AppendArgs schema (e.g., missing "path" or "content" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
