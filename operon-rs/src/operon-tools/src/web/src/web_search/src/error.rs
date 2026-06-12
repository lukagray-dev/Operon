//! Error types for the web_search tool.
//!
//! This module defines all error conditions that can occur during web_search tool
//! argument parsing. Per-search execution failures are NOT represented here — they
//! are returned as `Ok(ToolResult { is_error: true, ... })`.

use thiserror::Error;

/// Errors that can occur during web_search tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual search execution failures are captured in ToolResult, not here.
#[derive(Debug, Error)]
pub enum WebSearchToolError {
    /// Failed to parse the tool arguments from the plain-text attr map.
    ///
    /// This occurs when the model sends a call missing the required `query`
    /// attribute, or provides `max` with a non-integer value.
    /// The inner String is a human-readable description of what went wrong.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
