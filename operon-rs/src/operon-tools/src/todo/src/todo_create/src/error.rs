//! Error types for the todo_create tool.
//!
//! Defines error conditions that can occur during argument parsing.
//! Execution failures (validation errors, store operations) are returned
//! as ToolResult with is_error: true, not as errors here.

use thiserror::Error;

/// Errors that can occur during todo_create tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual validation failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum TodoCreateToolError {
    /// Failed to parse the tool arguments from the plain-text attr map.
    ///
    /// The inner String is a human-readable description of what went wrong.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
