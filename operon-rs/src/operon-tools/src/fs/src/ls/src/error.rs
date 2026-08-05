//! Error types for the ls tool.
//!
//! This module defines all error conditions that can occur during ls tool
//! argument parsing and execution. Per-entry listing failures are NOT represented
//! here — they are embedded in the error field of LsOutput.

use thiserror::Error;

/// Errors that can occur during ls tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual entry listing failures are captured in LsOutput.error, not here.
#[derive(Debug, Error)]
pub enum LsToolError {
    /// Failed to deserialize the tool arguments JSON into LsArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the LsArgs schema (e.g., missing "path" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
