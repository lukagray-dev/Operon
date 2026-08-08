//! Error types for the `ask` tool.
//!
//! Defines all error conditions that can occur during argument parsing and validation for the
//! `ask` tool. Execution does not have a separate error type — the runner handles
//! the suspension/resume cycle directly and constructs ToolResults itself.

use thiserror::Error;

/// Errors that can occur when the model calls the `ask` tool.
///
/// Argument parsing or option count validation can fail here. The actual "execution"
/// (suspending the loop and waiting for user input) is handled by the session runner, not this crate.
#[derive(Debug, Error)]
pub enum AskToolError {
    /// Failed to deserialize the tool arguments JSON into `AskArgs`.
    ///
    /// Common causes:
    /// - Missing `question` field.
    /// - Missing `options` field.
    /// - Wrong types (e.g. `question` is a number, not a string).
    #[error("failed to deserialize ask arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),

    /// `options` array does not contain exactly 3 elements.
    #[error("expected exactly 3 options, got {0}. Provide exactly 3 pre-defined answer options — the UI adds a 4th free-text field automatically.")]
    WrongOptionCount(usize),
}
