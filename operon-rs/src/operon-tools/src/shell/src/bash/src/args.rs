// args.rs — Argument types for the bash tool.
//
// Defines the defensive deserialization schema for the bash tool's input.
// The tool accepts a shell command, a required working directory (cwd),
// and an optional timeout in milliseconds, supporting common LLM parameter aliases.

use operon_tools_core::de::deserialize_flexible_u64_opt;
use serde::Deserialize;

// ─────────────────────────────────────────────────────────────────────────────
// BashArgs
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments for the bash tool.
///
/// All three fields are passed from the model as a JSON object.
/// `command` and `cwd` are required. `timeout_ms` is optional.
#[derive(Debug, Deserialize)]
pub struct BashArgs {
    /// The shell command to execute.
    #[serde(
        alias = "cmd",
        alias = "script",
        alias = "code",
        alias = "exec"
    )]
    pub command: String,

    /// Absolute path to the working directory for this command.
    #[serde(
        alias = "dir",
        alias = "directory",
        alias = "path",
        alias = "working_dir",
        alias = "working_directory",
        alias = "workingDir"
    )]
    pub cwd: String,

    /// Optional timeout in milliseconds. Supports integer 5000 or numeric string "5000".
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_u64_opt",
        alias = "timeout",
        alias = "timeoutMs"
    )]
    pub timeout_ms: Option<u64>,
}
