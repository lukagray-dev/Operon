//! # operon-tools-shell-bash
//!
//! Implements the `bash` tool for the Operon agent's shell group.
//!
//! Executes a shell command in a stateless subprocess and returns merged stdout+stderr,
//! exit code, and truncation status. Supports:
//! - Stateless execution: each call spawns a fresh `sh -c` subprocess
//! - Output capture: merged stdout and stderr, truncated to 10,000 characters
//! - Exit codes: 0 = success, non-zero = failure, -1 = timeout
//! - Optional timeout: specify timeout_ms to kill long-running commands
//! - Cross-platform: uses `sh -c` on Unix, `cmd /C` on Windows
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_shell_bash::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "command": "echo hello",
//!     "timeout_ms": 5000
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
use operon_tools_core::TieredToolDefinition;
use serde_json::json;

/// Returns the tiered tool definition for the `bash` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (stateless execution, output cap).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "command": {
                "type": "string",
                "description": "Shell command to execute. Runs in a fresh sh -c subprocess each call."
            },
            "timeout_ms": {
                "type": "integer",
                "minimum": 1,
                "description": "Optional timeout in milliseconds. No timeout if omitted."
            }
        },
        "required": ["command"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "bash".to_string(),
            description: "Executes a shell command in a stateless subprocess and returns merged \
                          stdout+stderr, exit code, and truncation status. Each call is independent — \
                          no state persists between calls. Chain commands with && or ; for sequential \
                          state. Output capped at 10,000 characters. Optionally specify timeout_ms."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "bash".to_string(),
            description: "\
Executes a shell command in a stateless subprocess and returns merged stdout+stderr, exit code, \
and truncation status. Each call is independent — no state (environment variables, working directory, \
shell variables) persists between calls.

## Input shapes

`command` (required, string): Shell command to execute. Runs in a fresh `sh -c` subprocess on Unix, \
`cmd /C` on Windows. The command is executed exactly as provided — no escaping or quoting is added. \
If the command is empty or whitespace-only, the tool returns an error.

`timeout_ms` (optional, integer, milliseconds): Optional timeout for the command. If provided, the \
command is killed if it exceeds this duration. If omitted, the command runs until completion with \
no timeout. There is no maximum — the model is responsible for setting a reasonable value for the task.

## Stateless execution model

Each call spawns a fresh subprocess. Working directory, environment variables, shell variables, and \
`cd` changes do NOT persist between calls. To chain commands that depend on prior state, use `&&` or `;` \
within a single `command` string.

### Example: stateless cd

Incorrect (cd does not persist):
```json
{
  \"command\": \"cd /tmp\"
}
```
Then in a separate call:
```json
{
  \"command\": \"pwd\"
}
```
Result: `pwd` returns the original working directory, NOT `/tmp`. The `cd` from the first call did not persist.

Correct (cd and pwd in one call):
```json
{
  \"command\": \"cd /tmp && pwd\"
}
```
Result: `pwd` returns `/tmp` because both commands run in the same subprocess.

## Output cap

Stdout and stderr are merged and truncated to 10,000 characters. When `truncated: true`, use more \
targeted commands to retrieve the specific part of the output needed:
- `| head -n 50` — first 50 lines
- `| tail -n 20` — last 20 lines
- `| grep \"pattern\"` — lines matching a pattern

### Example: large output

```json
{
  \"command\": \"python3 -c \\\"print('a' * 20000)\\\"\"
}
```
Result: `output` contains the first 10,000 characters, `truncated: true`.

To get the last part:
```json
{
  \"command\": \"python3 -c \\\"print('a' * 20000)\\\" | tail -c 1000\"
}
```

## Exit codes

- `exit_code: 0` — command succeeded
- `exit_code: N` (non-zero) — command reported failure. The command ran — the model receives the output \
  and decides what to do next. Non-zero exit is NOT a tool error.
- `exit_code: -1` — process was killed due to timeout (see `timed_out: true`)

Always check `exit_code` before treating output as valid.

## Timeout behavior

When `timed_out: true`, the process was killed and `output` contains whatever was buffered before the kill. \
`exit_code` will be -1. If the command is expected to take a long time, set `timeout_ms` appropriately.

### Example: timeout

```json
{
  \"command\": \"sleep 10\",
  \"timeout_ms\": 200
}
```
Result: `timed_out: true`, `exit_code: -1`, `output` is empty (sleep produces no output).

## No timeout by default

If `timeout_ms` is omitted, the command runs until completion. Use this for commands with unpredictable \
duration (builds, package installs). Set a timeout when you need a hard deadline.

## When to use bash vs fs tools

Prefer fs tools (`read`, `edit`, `write`, `grep`, `ls`) for file operations — they are faster, safer, \
and return structured output. Use `bash` for:
- Running build systems (make, cargo, npm, etc.)
- Package managers (apt, pip, npm install, etc.)
- Git operations (git clone, git commit, etc.)
- Test runners (pytest, cargo test, npm test, etc.)
- CLI tools and utilities
- Anything requiring shell features (pipes, environment variables, process management)

## Common mistakes

### Mistake #1: Expecting cd to persist
```json
{
  \"command\": \"cd /tmp\"
}
```
Then later:
```json
{
  \"command\": \"pwd\"
}
```
Result: `pwd` returns the original directory, NOT `/tmp`. Each call is stateless.

Fix: Use `cd /tmp && pwd` in a single call.

### Mistake #2: Running a command that produces massive output without piping
```json
{
  \"command\": \"cat /var/log/huge_file.log\"
}
```
Result: `output` is truncated to 10,000 characters. Important lines may be lost.

Fix: Use `| head -n 50` or `| tail -n 50` to get the specific part you need.

### Mistake #3: Forgetting to set timeout for long-running commands
```json
{
  \"command\": \"npm install\"
}
```
Result: The command may take minutes. If the model's context window expires, the call is abandoned.

Fix: Set `timeout_ms` to a reasonable value for the expected duration, or accept that long commands may not complete.

### Mistake #4: Empty command
```json
{
  \"command\": \"\"
}
```
Error: \"command is empty\"

Fix: Provide a non-empty command.

## Error messages

- \"command is empty\" → Provide a non-empty command.
- \"failed to spawn process: ...\" → OS-level error (permission denied, command not found, etc.). \
  The command was not executed.

## Output fields

- `command`: The command that was executed (echoed back for correlation).
- `exit_code`: Exit code of the process (0 = success, non-zero = failure, -1 = timeout).
- `output`: Merged stdout + stderr, truncated to 10,000 characters.
- `truncated`: True if the output was truncated.
- `timed_out`: True if the command was killed due to timeout."
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the bash tool.
///
/// Returns a `ToolResult` with either success (JSON BashOutput) or failure (Text error message).
/// Returns `Err(BashToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(BashToolError::ArgsParse)` if the arguments are malformed.
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
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: BashArgs = serde_json::from_value(args_json)?;

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
