// error.rs — Error types for the `spawn_agent` tool.
//
// Hey friend! Whenever something goes wrong when parsing arguments or preparing
// a subagent invocation, this module provides the clear, descriptive error types.

use thiserror::Error;

/// Errors that can occur during subagent tool invocation and argument parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpawnAgentToolError {
    /// The arguments passed to `spawn_agent` did not match the required JSON schema.
    #[error("failed to parse spawn_agent arguments: {0}")]
    ArgsParse(String),

    /// The subagent failed to execute or initialize.
    #[error("subagent execution failed: {0}")]
    Execution(String),
}

