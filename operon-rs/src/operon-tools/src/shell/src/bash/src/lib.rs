//! # operon-tools-shell-bash
//!
//! Implements the `bash` tool for the Operon agent's shell group.
//!
//! Executes a shell command in a stateless subprocess with an explicit working
//! directory (`path`) and returns merged stdout+stderr, exit code, and status markers
//! as plain text.
//!
//! ## Call format
//!
//! ```text
//! <bash path="C:\project">
//! <<<<
//! command="cargo build --release"
//! timeout="120000"
//! >>>>
//! ```
//!
//! `path` is the XML attribute (required). `command` and optional `timeout` are body keys.
//!
//! ## Why `path` is required
//!
//! The bash tool is directory-scoped in the Operon permission model. Every call
//! must declare the directory it operates in so `operon-policy` can enforce
//! per-directory shell permissions before the call reaches this tool.
//!
//! Without an explicit `path`, an external user could trigger shell execution
//! without providing an anchor for the policy check. Making it required closes
//! that attack surface at the model schema level — the model cannot omit it.
//!
//! ## Features
//!
//! - Stateless execution: each call spawns a fresh `sh -c` / `cmd /C` subprocess.
//! - Working directory: subprocess runs with `path` as its working directory.
//! - Output capture: merged stdout and stderr, truncated to 10,000 characters.
//! - Exit codes: 0 = success, non-zero = failure, -1 = timeout.
//! - Always-present timeout: defaults to 30 minutes; override with body `timeout` key.
//! - Cross-platform: uses `sh -c` on Unix, `cmd /C` on Windows.

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use error::BashToolError;

use args::BashArgs;
use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

// ─────────────────────────────────────────────────────────────────────────────
// definition
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the tiered tool definition for the `bash` tool.
///
/// # Tiers
///
/// - `short`: Sent under normal conditions. Concise description covering what the
///   tool does, its key constraints, and all required fields.
/// - `detailed`: Sent after a malformed call. Full description with call format,
///   path semantics, timeout behavior, exit codes, common mistakes, and examples.
pub fn definition() -> TieredToolDefinition {
    // The JSON schema declares only the `path` attribute field.
    // `command` and `timeout` live in the tool body, not the schema.
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to the working directory for this command."
            }
        },
        "required": ["path"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "bash".to_string(),
            description: "Executes a shell command in a stateless subprocess rooted at path. \
                          Write command=\"...\" and optional timeout=\"ms\" in the tool body. \
                          Each call is stateless — no state persists between calls, chain with \
                          && for sequential state. Output capped at 10,000 characters. \
                          Default timeout: 30 minutes; always set your own timeout for \
                          long-running commands."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "bash".to_string(),
            description: "\
Executes a shell command in a stateless subprocess rooted at the specified working directory (path). \
Returns merged stdout+stderr and exit code as plain text.

## Call format

<bash path=\"C:\\\\project\">
<<<<
command=\"cargo build --release\"
timeout=\"120000\"
>>>>

<bash path=\"C:\\\\project\">
<<<<
command=\"npm install && npm run build\"
>>>>

`path` (attr, required): Absolute path to the working directory. The subprocess is launched \
with this directory as its working directory. Must be:
- An absolute path (starts with `/` on Unix, drive letter on Windows).
- An existing directory on disk.
- Within an allowed directory per the active permission policy.

`command` (body key, required): Shell command to execute. Runs in a fresh `sh -c` subprocess \
on Unix, `cmd /C` on Windows. Empty or whitespace-only commands return an error.

`timeout` (body key, optional, milliseconds): The subprocess is killed after this many \
milliseconds. When killed, the output will include `***timed out***`. Default: 1,800,000 ms \
(30 minutes). Always set an explicit timeout for known-duration commands.

## Stateless execution model

Each call spawns a fresh subprocess. Working directory, environment variables, shell variables, and \
`cd` changes do NOT persist between calls. To chain commands that depend on prior state, use `&&` or `;` \
within a single `command` string.

### Example: stateless cd (wrong)

First call:
  command=\"cd /tmp\"

Second call:
  command=\"pwd\"
Result: `pwd` returns the `path` dir, NOT `/tmp`. The `cd` from the first call did not persist.

### Example: cd + pwd in one call (correct)

  command=\"cd /tmp && pwd\"
Result: `pwd` returns `/tmp` because both commands run in the same subprocess.

## Output cap

Stdout and stderr are merged and truncated to 10,000 characters. When `***truncated***` appears, \
use more targeted commands: `| head -n 50`, `| tail -n 20`, `| grep \"pattern\"`.

## Exit codes

- `exit: 0` — command succeeded.
- `exit: N` (non-zero) — command reported failure. The command ran — the model receives the \
  output and decides what to do next. Non-zero exit is NOT a tool error.
- `exit: -1` — process was killed due to timeout.

Always check the exit line before treating output as valid.

## Output format

`exit: {N}` on the first line, followed by merged stdout+stderr, followed by `***truncated***` if \
capped at 10,000 chars, and `***timed out***` if the process was killed.

## Common mistakes

### Mistake #1: Missing `command` body key

<bash path=\"C:\\\\project\">
<<<<
>>>>

Error: `command` body key is required. Always include `command=\"...\"` in the body.

### Mistake #2: Expecting `cd` to persist

  command=\"cd /tmp\"

Then separately:
  command=\"pwd\"

Result: `pwd` returns the `path` dir, NOT `/tmp`. Fix: use `cd /tmp && pwd` in one call.

### Mistake #3: Massive output without targeting

  command=\"cat /var/log/huge.log\"

Output will be truncated. Fix: pipe to `head`, `tail`, or `grep` to target the relevant lines.

### Mistake #4: Not setting timeout

  command=\"npm install\"

May run for minutes. The default is 30 minutes. For known-duration commands always set an \
explicit timeout to avoid stalling the agent loop."
                .to_string(),
            parameters,
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// execute
// ─────────────────────────────────────────────────────────────────────────────

/// Parses `args_json` and executes the bash tool.
///
/// Returns a `ToolResult` with either success (plain text output) or failure
/// (Text error message). Returns `Err(BashToolError::ArgsParse)` only if the
/// body-format arguments are invalid (missing `path` attr or `command` body key).
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model (with `path` and `__body__`).
///
/// # Returns
/// - `Ok(ToolResult)` — either success or an in-band error (both as `Ok`).
/// - `Err(BashToolError::ArgsParse)` — if required fields are missing or invalid.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, BashToolError> {
    // Parse args from the body-based format. Missing `path` or `command` → ArgsParse error.
    // This surfaces to the dispatcher which marks the tool as degraded.
    let args = BashArgs::parse(&args_json).map_err(BashToolError::ArgsParse)?;

    // Execute the tool. The executor handles all runtime validation
    // and always returns a ToolResult — it never panics or propagates errors.
    Ok(executor::execute(call_id, args).await)
}

/// Parses `args_json` and executes the bash tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, BashToolError> {
    // Parse args from the body-based format. Missing `path` or `command` → ArgsParse error.
    // This surfaces to the dispatcher which marks the tool as degraded.
    let args = BashArgs::parse(&args_json).map_err(BashToolError::ArgsParse)?;

    // Emit a progress event so the UI can show a "running" indicator while the command executes.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "bash",
            Some(args.path.clone()),
            format!("Running shell command in {}", args.path),
        ),
    );

    // Execute the tool. The executor handles all runtime validation
    // and always returns a ToolResult — it never panics or propagates errors.
    Ok(executor::execute(call_id, args).await)
}
