//! # operon-tools-fs-ls
//!
//! Implements the `ls` tool for the Operon agent's filesystem group.
//!
//! Lists files and directories at a given path. Supports:
//! - Recursive depth control (depth=1 default, depth=0 for unlimited)
//! - File name glob filtering (e.g., "*.py" lists only Python files)
//! - Entry ignore patterns (skip entries by name)
//! - 1000 entry limit to prevent overwhelming the model
//! - Human-readable file sizes
//! - Plain-text output with [DIR] and [FILE] prefixes
//!
//! ## Call format
//!
//! ```text
//! <!-- Simple: list immediate children -->
//! <ls path="C:\absolute\path\to\directory">
//!
//! <!-- With options: -->
//! <ls path="C:\absolute\path\to\directory">
//! <<<<
//! depth="2"
//! glob="*.py"
//! ignore="node_modules" ".git"
//! >>>>
//! ```

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::LsArgs;
pub use error::LsToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `ls` tool.
///
/// - `short`: sent to the model under normal conditions. Concise.
/// - `detailed`: sent after a malformed call. Full body format explanation.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "ls".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses `args_json` and executes the ls tool.
///
/// Returns a `ToolResult` with `is_error: false` always — directory errors are
/// embedded inline in the text output.
/// Returns `Err(LsToolError::ArgsParse)` only if the `path` attribute is missing
/// or the body contains an invalid value.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments sent by the parser.
///
/// # Returns
/// - `Ok(ToolResult)` with directory listing as plain text.
/// - `Err(LsToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, LsToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the ls tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, LsToolError> {
    // Parse the arguments from path attr + body.
    let args = LsArgs::parse(&args_json).map_err(LsToolError::ArgsParse)?;

    // Emit progress so the UI shows "Listing {path}" while waiting.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "ls",
            Some(args.path.clone()),
            format!("Listing {}", args.path),
        ),
    );

    // Execute the listing (always Ok — errors are inline in the text output).
    Ok(executor::execute(call_id, args).await)
}
