/// Error types for the read tool.
///
/// This module defines all error conditions that can occur during read tool
/// argument parsing and execution. Per-file read failures are NOT represented
/// here — they are embedded in the text output returned by the executor.
use thiserror::Error;

/// Errors that can occur during read tool execution.
///
/// These are top-level errors that prevent the tool from running at all.
/// Individual file read failures are captured inline in the text output,
/// not here. Only argument parse failures surface as ReadToolError.
#[derive(Debug, Error)]
pub enum ReadToolError {
    /// Failed to parse the tool arguments from the plain-text attr map.
    ///
    /// This occurs when the `paths` attribute is missing, not a string,
    /// contains an invalid path entry, or has a malformed range specification.
    /// The inner String is a human-readable description of what went wrong.
    #[error("failed to parse tool arguments: {0}")]
    ArgsParse(String),
}
