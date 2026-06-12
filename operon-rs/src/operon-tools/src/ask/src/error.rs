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
    /// Failed to parse the tool arguments from the body-based format.
    ///
    /// Common causes:
    /// - Missing `__body__` field entirely.
    /// - Missing `question` body key.
    /// - Missing `option1`, `option2`, or `option3` body key.
    #[error("failed to parse ask arguments: {0}")]
    ArgsParse(String),
}
