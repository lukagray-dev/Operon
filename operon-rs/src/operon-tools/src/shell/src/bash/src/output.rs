//! Output types for the bash tool.
//!
//! This module defines the structured result format returned by the bash tool
//! on successful completion. Failures use ToolContent::Text directly — no struct needed.

use serde::{Deserialize, Serialize};

/// Output returned to the model after a bash command executes.
///
/// Returned even when the command exits with a non-zero code or times out —
/// the model receives the output and exit code and decides how to proceed.
/// Only process spawn failures return `ToolResult { is_error: true }`.
#[derive(Debug, Serialize, Deserialize)]
pub struct BashOutput {
    /// The command that was executed (echoed back for correlation).
    pub command: String,

    /// Exit code of the process. 0 = success. Non-zero = failure.
    /// -1 if the process was killed due to timeout (see `timed_out`).
    pub exit_code: i32,

    /// Merged stdout + stderr output, truncated to MAX_OUTPUT_CHARS if needed.
    pub output: String,

    /// True if the output was truncated at MAX_OUTPUT_CHARS.
    /// When true, use more targeted commands (pipe to `head`, `tail`, `grep`)
    /// to retrieve the specific part of the output you need.
    pub truncated: bool,

    /// True if the command was killed because it exceeded the timeout_ms limit.
    pub timed_out: bool,
}
