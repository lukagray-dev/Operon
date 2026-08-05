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
    /// Failed to deserialize the tool arguments JSON into TodoCreateArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the TodoCreateArgs schema (e.g., missing "content" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
