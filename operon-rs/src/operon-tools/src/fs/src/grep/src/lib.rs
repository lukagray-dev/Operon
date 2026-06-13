//! # operon-tools-fs-grep
//!
//! Implements the `grep` tool for the Operon agent's filesystem group.
//!
//! Searches files and directories for regex patterns. Supports:
//! - Multiple OR-combined regex patterns (match if any pattern matches)
//! - Recursive directory walking with gitignore rules respected
//! - Filename glob filtering (e.g., "*.py" to search only Python files)
//! - Directory/entry ignore patterns
//! - Context lines before/after matches
//! - Per-file match reporting with line numbers
//! - 300 match limit to prevent context overflow
//! - 10 MB file size limit
//! - Binary file detection and skipping
//! - Glob-only mode (no patterns → lists matching files)
//!
//! ## Call format
//!
//! ```text
//! <grep path="C:\absolute\path\to\directory">
//!
//! <grep path="C:\absolute\path\to\directory">
//! <<<<
//! pattern="calculate_total"
//! glob="*.py"
//! ignore="node_modules" ".git"
//! context="3"
//! >>>>
//!
//! <!-- Glob-only: no pattern = lists matching files -->
//! <grep path="C:\absolute\path\to\directory">
//! <<<<
//! glob="*.py"
//! >>>>
//! ```

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::GrepArgs;
pub use error::GrepToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `grep` tool.
///
/// - `short`: sent to the model under normal conditions. Concise.
/// - `detailed`: sent after a malformed call. Full explanation with body format.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "grep".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses `args_json` and executes the grep tool.
///
/// Returns a `ToolResult` with `is_error: false` always — errors are embedded
/// in the plain-text output. Returns `Err(GrepToolError::ArgsParse)` only if
/// the `path` attribute is missing or the body is structurally invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments sent by the parser.
///
/// # Returns
/// - `Ok(ToolResult)` with search results as plain text.
/// - `Err(GrepToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, GrepToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the grep tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, GrepToolError> {
    // Parse the arguments from path attr + body.
    let args = GrepArgs::parse(&args_json).map_err(GrepToolError::ArgsParse)?;

    // Emit progress so the UI shows "Searching {path}" while waiting.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "grep",
            Some(args.path.clone()),
            format!("Searching {}", args.path),
        ),
    );

    // Execute the search and return the result (always Ok — errors are inline).
    Ok(executor::execute(call_id, args).await)
}
