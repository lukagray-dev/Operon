//! Argument types for the bash tool.
//!
//! This module defines the deserialization schema for the bash tool's input.
//! The tool accepts a shell command and an optional timeout in milliseconds.

use serde::Deserialize;

/// Arguments for the bash tool.
///
/// Specifies a shell command to execute and an optional timeout.
/// Each call is independent — no state (env vars, working directory, shell variables)
/// persists between calls. Use `&&` or `;` to chain multiple commands in a single call
/// when sequential state is needed.
#[derive(Debug, Deserialize)]
pub struct BashArgs {
    /// The shell command to execute. Runs in a stateless `sh -c` subprocess.
    /// Each call is independent — no state (env vars, working directory, shell
    /// variables) persists between calls. Use `&&` or `;` to chain multiple
    /// commands in a single call when sequential state is needed.
    pub command: String,

    /// Optional timeout in milliseconds. If not provided, the command runs
    /// until it completes with no timeout. Use this for long-running commands
    /// (builds, installs, tests) that are known to take more than a few seconds.
    /// There is no maximum — the model is responsible for setting a reasonable
    /// value for the task.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}
