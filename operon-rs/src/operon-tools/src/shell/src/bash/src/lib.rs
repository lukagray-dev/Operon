//! # operon-tools-shell-bash
//!
//! Implements the `bash` tool for the Operon agent's shell group.
//!
//! Executes a shell command in a stateless subprocess with an explicit working
//! directory (`cwd`) and returns merged stdout+stderr, exit code, and metadata.
//!
//! ## Why `cwd` is required
//!
//! The bash tool is directory-scoped in the Operon permission model. Every call
//! must declare the directory it operates in so `operon-policy` can enforce
//! per-directory shell permissions before the call reaches this tool.
//!
//! Without an explicit `cwd`, an external user could trigger shell execution
//! without providing an anchor for the policy check. Making it required closes
//! that attack surface at the model schema level — the model cannot omit it.
//!
//! ## Features
//!
//! - Stateless execution: each call spawns a fresh `sh -c` / `cmd /C` subprocess.
//! - Working directory: subprocess runs with `cwd` as its working directory.
//! - Output capture: merged stdout and stderr, truncated to 10,000 characters.
//! - Exit codes: 0 = success, non-zero = failure, -1 = timeout.
//! - Optional timeout: specify `timeout_ms` to kill long-running commands.
//! - Cross-platform: uses `sh -c` on Unix, `cmd /C` on Windows.
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_shell_bash::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Register the tool definition with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "command": "cargo build --release",
//!     "cwd": "/home/user/my-project",
//!     "timeout_ms": 120_000
//! });
//! let result = execute(
//!     ToolCallId("call_123".to_string()),
//!     args
//! ).await.unwrap();
//! # }
//! ```

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::BashArgs;
pub use error::BashToolError;
pub use output::BashOutput;

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
/// - `detailed`: Sent after a malformed call. Full description with input shapes,
///   cwd semantics, timeout behavior, exit codes, common mistakes, and examples.
///
/// # Breaking change note
///
/// `cwd` was added as a required field (previously the tool had no working directory
/// concept). All callers must provide `cwd`. This change was made to enable
/// directory-scoped shell permissions in `operon-policy`.
pub fn definition() -> TieredToolDefinition {
    // The JSON schema is shared between short and detailed definitions.
    // Only the description text differs between the two tiers.
    let parameters = json!({
        "type": "object",
        "properties": {
            "command": {
                "type": "string",
                "description": "Shell command to execute. Runs in a fresh sh -c subprocess each call. \
                                No state persists between calls — chain with && or ; for sequential state."
            },
            "cwd": {
                "type": "string",
                "description": "Absolute path to the working directory for this command. \
                                Must be within an allowed directory. The subprocess runs with \
                                this directory as its working directory."
            },
            "timeout_ms": {
                "type": "integer",
                "minimum": 1,
                "description": "Optional timeout in milliseconds. Process is killed if it \
                                exceeds this duration. No timeout if omitted."
            }
        },
        // Both command and cwd are required — cwd is the policy anchor.
        "required": ["command", "cwd"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "bash".to_string(),
            description: "Executes a shell command in a stateless subprocess rooted at `cwd` and \
                          returns merged stdout+stderr, exit code, and truncation status. Each call \
                          is independent — no state persists between calls. Chain commands with && \
                          or ; for sequential state within one call. Output capped at 10,000 \
                          characters. `cwd` (absolute path) and `command` are required. \
                          Optionally specify `timeout_ms`."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "bash".to_string(),
            description: "\
Executes a shell command in a stateless subprocess rooted at the specified working directory (`cwd`). \
Returns merged stdout+stderr, exit code, and execution metadata.

## Required fields

`command` (string): Shell command to execute. Runs in a fresh `sh -c` subprocess on Unix, `cmd /C` \
on Windows. Empty or whitespace-only commands return an error.

`cwd` (string): Absolute path to the working directory for this command. The subprocess is launched \
with this directory as its working directory. Must be:
- An absolute path (starts with `/` on Unix, drive letter on Windows).
- An existing directory on disk.
- Within an allowed directory per the active permission policy.

If `cwd` is missing, the call is rejected by the policy layer before reaching this tool.

## Optional fields

`timeout_ms` (integer, milliseconds): If provided, the subprocess is killed after this many \
milliseconds. When killed, `timed_out` is true and `exit_code` is -1.

## Stateless execution model

Each call spawns a fresh subprocess. Working directory, environment variables, shell variables, and \
`cd` changes do NOT persist between calls. To chain commands that depend on prior state, use `&&` or `;` \
within a single `command` string.

### Example: stateless cd (wrong)
```json
{ \"command\": \"cd /tmp\" }
```
Then in a separate call:
```json
{ \"command\": \"pwd\", \"cwd\": \"/home/user\" }
```
Result: `pwd` returns `/home/user` (the `cwd`), NOT `/tmp`. The `cd` from the first call did not persist.

### Example: cd + pwd in one call (correct)
```json
{ \"command\": \"cd /tmp && pwd\", \"cwd\": \"/home/user\" }
```
Result: `pwd` returns `/tmp` because both commands run in the same subprocess.

## Output cap

Stdout and stderr are merged and truncated to 10,000 characters. When `truncated: true`, use more \
targeted commands: `| head -n 50`, `| tail -n 20`, `| grep \"pattern\"`.

## Exit codes

- `exit_code: 0` — command succeeded.
- `exit_code: N` (non-zero) — command reported failure. The command ran — the model receives the \
  output and decides what to do next. Non-zero exit is NOT a tool error.
- `exit_code: -1` — process was killed due to timeout (`timed_out: true`).

Always check `exit_code` before treating output as valid.

## Output fields

- `command`: The command that was executed (echoed for correlation).
- `cwd`: The working directory the command ran in (echoed for correlation).
- `exit_code`: Exit code (0 = success, non-zero = failure, -1 = timeout).
- `output`: Merged stdout + stderr, truncated to 10,000 characters.
- `truncated`: True if output was truncated at 10,000 characters.
- `timed_out`: True if the process was killed by the timeout.

## Common mistakes

### Mistake #1: Missing `cwd`
```json
{ \"command\": \"ls\" }
```
Error: `cwd` is required. Always provide an absolute path.

### Mistake #2: Expecting `cd` to persist
```json
{ \"command\": \"cd /tmp\", \"cwd\": \"/home/user\" }
```
Then separately:
```json
{ \"command\": \"pwd\", \"cwd\": \"/home/user\" }
```
Result: `pwd` returns `/home/user`, NOT `/tmp`. Fix: use `cd /tmp && pwd` in one call.

### Mistake #3: Massive output without targeting
```json
{ \"command\": \"cat /var/log/huge.log\", \"cwd\": \"/home/user\" }
```
Output will be truncated. Fix: pipe to `head`, `tail`, or `grep` to target the relevant lines.

### Mistake #4: Long-running command without timeout
```json
{ \"command\": \"npm install\", \"cwd\": \"/home/user/project\" }
```
May run for minutes. Fix: set `timeout_ms` to a reasonable deadline for the expected duration."
                .to_string(),
            parameters,
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// execute
// ─────────────────────────────────────────────────────────────────────────────

/// Deserializes `args_json` and executes the bash tool.
///
/// Returns a `ToolResult` with either success (JSON `BashOutput`) or failure
/// (Text error message). Returns `Err(BashToolError::ArgsParse)` only if the
/// top-level JSON shape is invalid (i.e. missing required fields).
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` — either success or an in-band error (both as `Ok`).
/// - `Err(BashToolError::ArgsParse)` — if `command` or `cwd` are missing/wrong type.
///
/// # Example
/// ```rust
/// # use operon_tools_shell_bash::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({
///         "command": "echo hello",
///         "cwd": "/tmp",
///         "timeout_ms": 5000
///     })
/// ).await.unwrap();
/// assert_eq!(result.name, "bash");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, BashToolError> {
    // Deserialize args. Missing `command` or `cwd` → ArgsParse error.
    // This surfaces to the dispatcher which marks the tool as degraded.
    let args: BashArgs = serde_json::from_value(args_json)?;

    // Execute the tool. The executor handles all runtime validation
    // and always returns a ToolResult — it never panics or propagates errors.
    Ok(executor::execute(call_id, args).await)
}

/// Deserializes `args_json` and executes the bash tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, BashToolError> {
    // Deserialize args. Missing `command` or `cwd` â†’ ArgsParse error.
    // This surfaces to the dispatcher which marks the tool as degraded.
    let args: BashArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "bash",
            Some(args.cwd.clone()),
            format!("Running shell command in {}", args.cwd),
        ),
    );

    // Execute the tool. The executor handles all runtime validation
    // and always returns a ToolResult â€” it never panics or propagates errors.
    Ok(executor::execute(call_id, args).await)
}
