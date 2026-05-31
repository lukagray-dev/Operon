//! Error types for the web_search tool.
//!
//! This module defines all error conditions that can occur during web_search tool
//! argument parsing. Per-search execution failures are NOT represented here — they are
//! returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during web_search tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual search execution failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum WebSearchToolError {
    /// Failed to deserialize the tool arguments JSON into WebSearchArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the WebSearchArgs schema (e.g., missing "query" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
