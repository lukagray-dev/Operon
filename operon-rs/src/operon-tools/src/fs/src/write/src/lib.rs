//! # operon-tools-fs-write
//!
//! Implements the `write` tool for the Operon agent's filesystem group.
//!
//! Writes a new file or completely overwrites an existing file with atomic writes.
//! Supports:
//! - Creating new files (parent directories are auto-created if needed)
//! - Overwriting existing files (complete replacement, not append)
//! - Atomic writes (temp file + rename — if it fails, original file untouched)
//!
//! ## Call format
//!
//! ```
//! <write path="C:\absolute\path\to\file.txt">
//! <<<<
//! raw file content here
//! with real line breaks
//! >>>>
//! ```
//!
//! The dispatcher injects:
//! - `args_json["path"]`     — the absolute file path from the `path` attr.
//! - `args_json["__body__"]` — the raw body content between the tag and `>>>>`.
//!
//! ## Output format (plain text)
//!
//! - New file:   `"{path} created"`
//! - Overwrite:  `"{path} overwritten"`
//! - Any error:  `"{path}\nERROR: {reason}"`

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

// Export the Args and Error types. WriteOutput no longer exists.
pub use args::WriteArgs;
pub use error::WriteToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `write` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints.
/// - `detailed`: sent after a malformed call. Full explanation with call format,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "write".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses `args_json` and executes the write tool.
///
/// Returns a `ToolResult` with ToolContent::Text for both success and error.
/// Returns `Err(WriteToolError::ArgsParse)` only if the required `path` attr is
/// missing or malformed — all other errors are returned as Ok(ToolResult).
///
/// # Arguments
/// - `call_id`:   The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments injected by the dispatcher. Must contain
///                `"path"` (String) and optionally `"__body__"` (String).
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, WriteToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the write tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, WriteToolError> {
    // Parse arguments manually (no serde Deserialize).
    // A missing or empty "path" is the only hard failure — body absence is allowed.
    let args = WriteArgs::parse(&args_json).map_err(WriteToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "write",
            Some(args.path.clone()),
            format!("Writing {}", args.path),
        ),
    );

    // Execute and return the result. The executor always returns Ok(ToolResult).
    Ok(executor::execute(call_id, args).await)
}
