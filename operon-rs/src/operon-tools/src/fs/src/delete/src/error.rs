//! Error types for the delete tool.
//!
//! This module defines all error conditions that can occur during delete tool
//! argument parsing. Per-file deletion failures are NOT represented here — they
//! are embedded inline in the plain-text output returned by the executor.

use thiserror::Error;

/// Errors that can occur during delete tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual file deletion failures are formatted inline in the output text.
#[derive(Debug, Error)]
pub enum DeleteToolError {
    /// Failed to parse the tool arguments from the attrs+body map.
    ///
    /// This occurs when the `path` attribute is missing, not a string,
    /// or when the body contains an invalid value (e.g., non-boolean permanent).
    /// The inner String is a human-readable description of what went wrong.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
