//! Error types for the edit tool.
//!
//! Hey friend! This module defines the top-level error conditions that can occur
//! during edit tool argument parsing and execution setup.
//!
//! Runtime execution failures (e.g. hunk matching errors) are captured within `ToolResult`
//! rather than as top-level `Result::Err`, ensuring the model receives actionable diagnostics.

use thiserror::Error;

/// Errors that can occur during edit tool execution setup.
#[derive(Debug, Error)]
pub enum EditToolError {
    /// Failed to deserialize the tool arguments JSON into `EditArgs`.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the `EditArgs` schema (e.g., missing "path" or "edits" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
