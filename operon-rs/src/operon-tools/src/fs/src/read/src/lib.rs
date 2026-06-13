//! # operon-tools-fs-read
//!
//! Implements the `read` tool for the Operon agent's filesystem group.
//!
//! Reads one or multiple files in a single tool call. Supports:
//! - Multi-file batched reads (concurrent, up to 16 at a time)
//! - Chunked reading via line range syntax (e.g. `file.txt:40-90`, `file.txt:50-`, `file.txt:-30`)
//! - Binary file detection (null bytes → error)
//! - 1 MB size limit on full-file reads (range reads bypass it)
//! - CRLF normalization (\r\n → \n, standalone \r → \n)
//! - Per-file error inline in the text output (never a top-level is_error: true)
//! - Line-numbered output for every content line (e.g. "42| def foo():")
//!
//! ## Argument format
//!
//! The `paths` attribute is a single string containing a whitespace-separated list
//! of path entries. Each quoted value in the original call is joined with a space
//! by the dispatcher before arriving here. Each token is one path entry:
//!
//! ```text
//! C:\file.txt                   → full file read
//! C:\file.txt:40-90             → lines 40 to 90 inclusive (1-indexed)
//! C:\file.txt:50-               → line 50 to EOF
//! C:\file.txt:-30               → line 1 to line 30
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use operon_tools_fs_read::{definition, execute};
//! use operon_context_normalize::tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "paths": r"C:\src\main.rs C:\Cargo.toml"
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

pub use args::{ReadArgs, ReadTarget};
pub use error::ReadToolError;
// FileReadResult and LineRange are internal — no longer re-exported.
// (ReadOutput has been removed; the read tool now outputs plain text.)

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `read` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the single most important constraint (1 MB limit, range syntax).
/// - `detailed`: sent after a malformed call. Full explanation with the paths attr
///   format, range syntax rules, output format, and error behavior.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "read".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses `args_json` and executes the read tool.
///
/// Returns a `ToolResult` with `is_error: false` even on partial file failures —
/// per-file errors are embedded inline in the text content.
/// Returns `Err(ReadToolError::ArgsParse)` only if the top-level argument shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments sent by the parser (all values are strings).
///
/// # Returns
/// - `Ok(ToolResult)` with per-file results in plain text content.
/// - `Err(ReadToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, ReadToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the read tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, ReadToolError> {
    // Parse the arguments manually — no serde deserialization.
    // ReadArgs::parse returns Err(String) on any parse failure.
    let args = match ReadArgs::parse(&args_json) {
        Ok(a) => a,
        Err(reason) => return Err(ReadToolError::ArgsParse(reason)),
    };

    // Emit a progress event so the UI can show "Reading N file(s)" while waiting.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "read",
            None,
            format!("Reading {} file(s)", args.targets.len()),
        ),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error internally), so we wrap in Ok.
    Ok(executor::execute(call_id, args).await)
}
