// args.rs — Argument types for the bash tool.
//
// Defines the deserialization schema for the bash tool's input.
// The tool accepts a shell command, a required working directory (cwd),
// and an optional timeout in milliseconds.
//
// DESIGN NOTE — why `cwd` is required:
//   The bash tool is directory-scoped in the Operon permission model.
//   Each tool call must declare the directory it operates in so the
//   policy layer (operon-policy) can check the call against the
//   DirectoryPolicy for that path before dispatching.
//
//   If `cwd` were optional or derived from the session workspace root,
//   a malicious external user could omit it and escape per-directory
//   shell restrictions. Making it required and enforced by policy
//   closes that attack surface completely.

use serde::Deserialize;

// ─────────────────────────────────────────────────────────────────────────────
// BashArgs
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments for the bash tool.
///
/// All three fields are passed from the model as a JSON object.
/// `command` and `cwd` are required. `timeout_ms` is optional.
///
/// # Policy integration
///
/// The `cwd` field is the anchor for operon-policy's directory-scope check.
/// Before the dispatcher even calls `execute()`, the policy resolver extracts
/// `cwd` from the raw `ToolCall.arguments` JSON and resolves it against the
/// registered `DirectoryPolicy` entries. If no policy entry covers `cwd`,
/// the call is denied before reaching this struct.
///
/// This means by the time `BashArgs` is deserialized here, the `cwd` has
/// already been validated to be within an allowed directory for this caller role.
/// The executor still validates it exists on disk as a defence-in-depth measure.
#[derive(Debug, Deserialize)]
pub struct BashArgs {
    /// The shell command to execute.
    ///
    /// Runs in a fresh subprocess using the shell appropriate for the OS:
    /// - Unix: `sh -c <command>`
    /// - Windows: `cmd /C <command>`
    ///
    /// Each call is stateless — no environment variables, working directory
    /// changes, or shell state from previous calls persists. Chain sequential
    /// commands with `&&` or `;` within a single call when state is needed.
    pub command: String,

    /// Absolute path to the working directory for this command.
    ///
    /// This field is **required** — it is the policy anchor that allows the
    /// permission layer to enforce directory-scoped shell permissions.
    ///
    /// Rules:
    /// - Must be an absolute path (starts with `/` on Unix, drive letter on Windows).
    /// - Must refer to an existing directory on disk.
    /// - Must be within an allowed directory per the active `PolicyConfig`.
    ///
    /// The subprocess is spawned with this directory as its working directory.
    /// Relative paths inside the command (e.g. `./build.sh`) resolve against it.
    pub cwd: String,

    /// Optional timeout in milliseconds.
    ///
    /// If provided, the subprocess is killed after this many milliseconds.
    /// `exit_code` becomes -1 and `timed_out` becomes true in the output.
    ///
    /// If omitted, the command runs until it completes with no deadline.
    /// There is no maximum — the model is responsible for setting a reasonable
    /// value. For long-running commands (builds, installs, test suites),
    /// always set a timeout to avoid orphaned subprocesses.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}
