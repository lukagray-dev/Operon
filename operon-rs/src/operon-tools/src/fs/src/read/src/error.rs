/// Error types for the read tool.
///
/// This module defines all error conditions that can occur during read tool
/// argument parsing and execution. Per-file read failures are NOT represented
/// here — they are embedded in the success/error fields of FileReadResult.
use thiserror::Error;

/// Errors that can occur during read tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual file read failures are captured in FileReadResult, not here.
#[derive(Debug, Error)]
pub enum ReadToolError {
    /// Failed to deserialize the tool arguments JSON into ReadArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the ReadArgs schema (e.g., missing "paths" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
