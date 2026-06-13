//! # operon-tools-fs-edit
//!
//! Implements the `edit` tool for the Operon agent's filesystem group.
//!
//! Edits an existing file using a diff-based body format. Supports:
//! - Multi-hunk edits (one or more `@@`-delimited hunks per call)
//! - Fuzzy line matching via `seek_sequence` (exact → rstrip → trim → unicode-normalised)
//! - Optional per-hunk seek anchors (`@@ some context line`)
//! - EOF-anchored hunks (`*** End of File` marker)
//! - Overlap detection across hunks
//! - Atomic writes (all hunks applied or none)
//!
//! ## Call format
//!
//! ```text
//! <edit path="C:\absolute\path\to\file.rs">
//! <<<<
//! @@
//! -old line
//! +new line
//! >>>>
//! ```
//!
//! The dispatcher injects:
//! - `args_json["path"]`     — the absolute file path from the `path` XML attr.
//! - `args_json["__body__"]` — the raw diff body between `<<<<` and `>>>>`.

mod args;
mod error;
mod executor;
mod seek_sequence;

#[cfg(test)]
mod tests;

// Keep EditArgs accessible to callers that construct it directly (e.g. integration tests).
// EditHunk is internal — callers only interact with EditArgs.
pub use args::EditArgs;
pub use error::EditToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `edit` tool.
///
/// - `short`:    sent to the model under normal conditions. Concise summary of
///               the tool's purpose and the diff body format.
/// - `detailed`: sent after a malformed call. Full explanation of the call
///               format, hunk syntax, error messages, and worked examples.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "edit".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses `args_json` and executes the edit tool.
///
/// Returns a `ToolResult` with `ToolContent::Text` for both success and error.
/// Returns `Err(EditToolError::ArgsParse)` only if the `path` attr is missing
/// or the `__body__` diff body is absent or contains no valid hunks.
///
/// # Arguments
/// - `call_id`:   The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments injected by the dispatcher. Must contain
///                `"path"` (String) and `"__body__"` (String with diff body).
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, EditToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the edit tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, EditToolError> {
    // Parse arguments manually (no serde Deserialize).
    // A missing path or unparseable body is the only hard failure — all hunk
    // match errors are returned as Ok(ToolResult) with inline text.
    let args = EditArgs::parse(&args_json).map_err(EditToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "edit",
            Some(args.path.clone()),
            format!("Editing {} ({} hunk(s))", args.path, args.hunks.len()),
        ),
    );

    // Execute and return. The executor always returns Ok(ToolResult) — it never
    // panics or propagates an Err from file I/O (errors become inline text).
    Ok(executor::execute(call_id, args).await)
}
