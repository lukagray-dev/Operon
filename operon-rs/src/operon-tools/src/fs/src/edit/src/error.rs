//! Error types for the edit tool.
//!
//! This module defines all error conditions that can occur during edit tool
//! argument parsing. Per-hunk edit failures are NOT represented here — they
//! are returned as `Ok(ToolResult { is_error: false, content: ToolContent::Text(...) })`
//! so the model can read the inline error text and recover.

use thiserror::Error;

/// Errors that can occur during edit tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual hunk match failures are captured in ToolResult as plain text,
/// not here.
#[derive(Debug, Error)]
pub enum EditToolError {
    /// Failed to parse the tool arguments from the injected serde_json::Value.
    ///
    /// This occurs when the dispatcher did not inject a valid "path" string,
    /// the "__body__" field is absent or malformed, or the diff body contains
    /// no valid hunks. The inner String is a human-readable description of
    /// the problem.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
