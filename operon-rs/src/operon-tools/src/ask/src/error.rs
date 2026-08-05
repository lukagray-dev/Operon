//! Error types for the `ask` tool.
//!
//! Defines all error conditions that can occur during argument parsing for the
//! `ask` tool. Execution does not have a separate error type — the runner handles
//! the suspension/resume cycle directly and constructs ToolResults itself.

use thiserror::Error;

/// Errors that can occur when the model calls the `ask` tool.
///
/// Only argument parsing can fail here. The actual "execution" (suspending the
/// loop and waiting for user input) is handled by the session runner, not this crate.
#[derive(Debug, Error)]
pub enum AskToolError {
    /// Failed to deserialize the tool arguments JSON into `AskArgs`.
    ///
    /// Common causes:
    /// - Missing `question` field.
    /// - Missing `options` field.
    /// - `options` array has fewer or more than 3 elements.
    /// - Wrong types (e.g. `question` is a number, not a string).
    #[error("failed to deserialize ask arguments: {0}")]
    ArgsParse(#[from] serde_json::Error),
}
