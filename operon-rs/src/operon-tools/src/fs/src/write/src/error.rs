//! Error types for the write tool.
//!
//! This module defines all error conditions that can occur during write tool
//! argument parsing. Per-file write failures are NOT represented here — they
//! are returned as `Ok(ToolResult { is_error: false, content: ToolContent::Text(...) })`.

use thiserror::Error;

/// Errors that can occur during write tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual file write failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum WriteToolError {
    /// Failed to parse the tool arguments from the injected serde_json::Value.
    ///
    /// This occurs when the dispatcher did not inject a valid "path" string,
    /// or the value shape is otherwise unusable (e.g., missing required attr).
    /// The inner String is a human-readable description of the problem.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
