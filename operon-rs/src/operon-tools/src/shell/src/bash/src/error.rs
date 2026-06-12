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
    /// Failed to parse the tool arguments from the body-based format.
    ///
    /// This occurs when the model omits a required field (e.g. `path` attr or
    /// `command` body key), sends a non-parseable `timeout` value, or provides
    /// an empty `path` or `command`.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
