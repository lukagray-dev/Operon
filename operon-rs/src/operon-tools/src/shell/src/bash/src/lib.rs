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

#[cfg(test)]
mod tests;

pub use error::BashToolError;

use args::BashArgs;
use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

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
    TieredToolDefinition {
        short: ToolDefinition {
            name: "bash".to_string(),
            description: include_str!("description.md").to_string(),
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
