//! # operon-tools-fs-ls
//!
//! Implements the `ls` tool for the Operon agent's filesystem group.
//!
//! Lists files and directories at a given path (single level, not recursive).
//! Supports:
//! - Single-level directory listing with entry type prefixes (FILE/DIR/SYMLINK)
//! - Metadata collection (size, last-modified time)
//! - Glob-pattern exclusion by entry name
//! - 1000 entry limit to prevent overwhelming the model
//! - Per-entry error handling (missing metadata doesn't fail the entire listing)
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_fs_ls::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "path": "/home/user/project",
//!     "ignore": ["*.lock", "node_modules", ".git"]
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

pub use args::LsArgs;
pub use error::LsToolError;
pub use output::{EntryKind, LsEntry, LsOutput};

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `ls` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the key constraints (1000 entry limit, single-level only).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   return format, sort order, edge cases, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute or relative path to the directory to list. Defaults to '.' (current directory)."
            },
            "ignore": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Glob patterns matched against entry names to exclude (e.g. [\"*.lock\", \"node_modules\"])."
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "ls".to_string(),
            description: "Lists files and directories at a given path (single level). \
                          Pass `path` (directory path, defaults to '.'). \
                          Use `ignore` to exclude entries by glob. Returns plain text list."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "ls".to_string(),
            description: "\
Lists files and directories at a given path (single level, non-recursive). Returns plain text.

## Input shapes

1. Basic listing:
   `{\"path\": \"src\"}` or `{}` (defaults to current directory '.')

2. With ignore glob filters:
   `{\"path\": \"src\", \"ignore\": [\"*.lock\", \"node_modules\", \".git\"]}`

## Response format

Returns plain text list:
=== src (3 items) ===
[DIR]  subfolder/
[FILE] main.rs (1.2 KB)
[FILE] lib.rs (450 B)"
                .to_string(),
            parameters,
        },
    }
}


/// Deserializes `args_json` and executes the ls tool.
///
/// Returns a `ToolResult` with `is_error: false` even on directory listing failures —
/// per-directory errors are embedded in the JSON content.
/// Returns `Err(LsToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with directory listing results in JSON content.
/// - `Err(LsToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_fs_ls::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({ "path": "/tmp" })
/// ).await.unwrap();
/// assert_eq!(result.name, "ls");
/// assert!(!result.is_error);
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, LsToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Deserializes `args_json` and executes the ls tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, LsToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: LsArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "ls",
            Some(args.path.clone()),
            format!("Listing {}", args.path),
        ),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
