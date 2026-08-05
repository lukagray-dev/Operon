//! Error types for the bash tool.
//!
//! This module defines all error conditions that can occur during bash tool
//! argument parsing. Per-command execution failures are NOT represented here — they are
//! returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during bash tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual command execution failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum BashToolError {
    /// Failed to deserialize the tool arguments JSON into BashArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the BashArgs schema (e.g., missing "command" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
