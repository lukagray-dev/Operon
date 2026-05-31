//! Error types for the web_fetch tool.
//!
//! This module defines all error conditions that can occur during web_fetch tool
//! argument parsing. Per-fetch execution failures are NOT represented here — they are
//! returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during web_fetch tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual fetch execution failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum WebFetchToolError {
    /// Failed to deserialize the tool arguments JSON into WebFetchArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the WebFetchArgs schema (e.g., missing "url" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
