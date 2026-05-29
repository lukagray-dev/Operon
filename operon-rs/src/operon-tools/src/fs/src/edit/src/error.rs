//! Error types for the edit tool.
//!
//! This module defines all error conditions that can occur during edit tool
//! argument parsing and execution. Per-hunk edit failures are NOT represented
//! here — they are returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during edit tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual hunk edit failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum EditToolError {
    /// Failed to deserialize the tool arguments JSON into EditArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the EditArgs schema (e.g., missing "path" or "edits" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
