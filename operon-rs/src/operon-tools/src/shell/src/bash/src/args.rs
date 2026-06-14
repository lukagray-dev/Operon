// args.rs — Argument types for the bash tool.
//
// Defines the parsing logic for the bash tool's input in the new body-based format.
// The tool receives `path` as an XML attribute and `command` / `timeout` in the body.
//
// NEW CALL FORMAT:
//   <bash path="C:\absolute\path\to\directory">
//   <<<<
//   command="cargo build --release"
//   timeout="60000"
//   >>>>
//
// DESIGN NOTE — why `path` is required:
//   The bash tool is directory-scoped in the Operon permission model.
//   Each tool call must declare the directory it operates in so the
//   policy layer (operon-policy) can check the call against the
//   DirectoryPolicy for that path before dispatching.
//
//   If `path` were optional or derived from the session workspace root,
//   a malicious external user could omit it and escape per-directory
//   shell restrictions. Making it required and enforced by policy
//   closes that attack surface completely.

// ─────────────────────────────────────────────────────────────────────────────
// BashArgs
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments for the bash tool, parsed from the new body-based call format.
///
/// `path` arrives as `args_json["path"]` (the XML attribute).
/// `command` and optional `timeout` arrive as `args_json["__body__"]` key=value lines.
///
/// # Policy integration
///
/// The `path` field is the anchor for operon-policy's directory-scope check.
/// Before the dispatcher even calls `execute()`, the policy resolver extracts
/// `path` from the raw `ToolCall.arguments` JSON and resolves it against the
/// registered `DirectoryPolicy` entries. If no policy entry covers `path`,
/// the call is denied before reaching this struct.
///
/// This means by the time `BashArgs` is parsed here, the `path` has
/// already been validated to be within an allowed directory for this caller role.
/// The executor still validates it exists on disk as a defence-in-depth measure.
#[derive(Debug)]
pub struct BashArgs {
    /// Working directory for the command (was `cwd`, now from the `path` attr).
    pub path: String,

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

    /// Timeout in milliseconds. Defaults to 1_800_000 (30 minutes) if not specified.
    ///
    /// The subprocess is killed after this many milliseconds.
    /// `exit_code` becomes -1 and `timed_out` becomes true in the output.
    ///
    /// Always set an explicit timeout for long-running commands to avoid
    /// orphaned subprocesses and stalled agent loops.
    pub timeout_ms: u64,
}

impl BashArgs {
    /// Parse arguments from the attrs JSON format.
    ///
    /// `path` is taken from `args_json["path"]` (the XML attribute field).
    /// `command` is taken from `args_json["command"]`.
    /// `timeout` is taken from `args_json["timeout"]` or `args_json["timeout_ms"]`.
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` with a descriptive message if:
    /// - `path` is missing, non-string, or empty.
    /// - `command` is missing, non-string, or empty.
    /// - `timeout`/`timeout_ms` is present but not a valid u64.
    pub fn parse(args_json: &serde_json::Value) -> Result<BashArgs, String> {
        // ── Extract the `path` XML attribute ─────────────────────────────────
        let path = args_json["path"]
            .as_str()
            .ok_or_else(|| "missing or non-string attr: path".to_string())?
            .trim()
            .to_string();

        if path.is_empty() {
            return Err("path is empty".to_string());
        }

        // ── Extract the `command` attribute ──────────────────────────────────
        let command = args_json
            .get("command")
            .ok_or_else(|| "missing or non-string attr: command".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'command' must be a string".to_string())?
            .trim()
            .to_string();

        if command.is_empty() {
            return Err("command is empty".to_string());
        }

        // ── Extract the optional `timeout` or `timeout_ms` attribute ──────────
        let mut timeout_ms: u64 = 1_800_000;
        if let Some(v) = args_json.get("timeout").or_else(|| args_json.get("timeout_ms")) {
            let val = v.as_str().ok_or_else(|| "timeout must be a string".to_string())?.trim();
            timeout_ms = val.parse::<u64>().map_err(|_| {
                format!("invalid timeout value '{}': must be a non-negative integer", val)
            })?;
        }

        Ok(BashArgs {
            path,
            command,
            timeout_ms,
        })
    }
}
