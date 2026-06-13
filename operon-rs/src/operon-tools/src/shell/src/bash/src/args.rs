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
    /// Parse arguments from the new body-based JSON format.
    ///
    /// `path` is taken from `args_json["path"]` (the XML attribute field).
    /// `command` and optional `timeout` are parsed from `args_json["__body__"]`
    /// as key=value lines (one per line, e.g. `command="cargo build"`).
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` with a descriptive message if:
    /// - `path` is missing, non-string, or empty.
    /// - `command` is missing from the body or empty.
    /// - `timeout` is present but not a valid u64.
    pub fn parse(args_json: &serde_json::Value) -> Result<BashArgs, String> {
        // ── Extract the `path` XML attribute ─────────────────────────────────
        // This is always provided as a top-level JSON field by the parser.
        let path = args_json["path"]
            .as_str()
            .ok_or_else(|| "missing or non-string attr: path".to_string())?
            .trim()
            .to_string();

        if path.is_empty() {
            return Err("path is empty".to_string());
        }

        // ── Parse the body key=value lines ───────────────────────────────────
        // The body is the content between <<<< and >>>> markers in the call format.
        // It is passed as `args_json["__body__"]`. If missing, treat as empty.
        let body = args_json["__body__"].as_str().unwrap_or("");

        let mut command: Option<String> = None;
        // Default timeout is 30 minutes (1,800,000 ms). The model may override via body.
        let mut timeout_ms: u64 = 1_800_000;

        for line in body.lines() {
            let line = line.trim();
            // Skip blank lines gracefully — body may have padding.
            if line.is_empty() {
                continue;
            }

            // Each non-empty line must be in `key=value` form.
            if let Some(eq) = line.find('=') {
                let key = line[..eq].trim();
                let val = unquote_value(line[eq + 1..].trim());

                match key {
                    "command" => command = Some(val),
                    "timeout" => {
                        // Timeout must be a valid non-negative integer (milliseconds).
                        timeout_ms = val.parse::<u64>().map_err(|_| {
                            format!("invalid timeout value: {}", val)
                        })?;
                    }
                    // Unknown keys are silently ignored — forward compatibility.
                    _ => {}
                }
            }
        }

        // `command` is required — missing it is always a model error.
        let command = command.ok_or_else(|| "missing body key: command".to_string())?;

        if command.trim().is_empty() {
            return Err("command is empty".to_string());
        }

        Ok(BashArgs {
            path,
            command,
            timeout_ms,
        })
    }
}

/// Helper to strip enclosing double quotes and unescape internal quotes/backslashes.
fn unquote_value(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        let mut res = String::with_capacity(inner.len());
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next_c) = chars.peek() {
                    if next_c == '"' || next_c == '\\' {
                        res.push(next_c);
                        chars.next();
                        continue;
                    }
                }
            }
            res.push(c);
        }
        res
    } else {
        s.to_string()
    }
}
