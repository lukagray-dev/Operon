/// Error types for the grep tool.
///
/// This module defines all error conditions that can occur during grep tool
/// argument parsing. Per-file search failures are NOT represented here — they
/// are embedded in the FileGrepResult structure with an error field.

use thiserror::Error;

/// Errors that can occur during grep tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual file search failures are captured in FileGrepResult, not here.
#[derive(Debug, Error)]
pub enum GrepToolError {
    /// Failed to deserialize the tool arguments JSON into GrepArgs.
    ///
    /// This occurs when the model sends malformed JSON or a shape that doesn't
    /// match the GrepArgs schema (e.g., missing "pattern" or "paths" field, wrong types).
    #[error("failed to deserialize tool arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
