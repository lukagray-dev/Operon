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
Executes a shell command in a stateless subprocess rooted at `cwd`. Returns merged stdout+stderr (max 10,000 chars) and exit code.

## Input shapes

1. Basic command execution:
   `{\"command\": \"cargo check\", \"cwd\": \"/path/to/project\"}`

2. Command with timeout (in milliseconds):
   `{\"command\": \"npm test\", \"cwd\": \"/path/to/project\", \"timeout_ms\": 30000}`

3. Chaining sequential commands in one subprocess:
   `{\"command\": \"cd /tmp && pwd\", \"cwd\": \"/home/user\"}`

## Key behavior

- Stateless: Each call runs in a fresh process. `cd` and env vars do NOT persist across calls — chain with `&&` or `;`.
- `cwd` required: Must be an absolute path to the working directory.
- Output: Merged stdout+stderr truncated at 10,000 characters."
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
/// Returns a `ToolResult` with either success (plain-text `BashOutput`) or failure
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
