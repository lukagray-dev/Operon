//! # operon-tools-fs-write
//!
//! Implements the `write` tool for the Operon agent's filesystem group.
//!
//! Writes a new file or completely overwrites an existing file with atomic writes.
//! Supports:
//! - Creating new files
//! - Overwriting existing files (complete replacement, not append)
//! - Atomic writes (temp file + rename pattern — if it fails, original file untouched)
//! - Validation that parent directory exists (does not create intermediate directories)
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_fs_write::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "path": "/path/to/file.txt",
//!     "content": "Hello, world!"
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

pub use args::WriteArgs;
pub use error::WriteToolError;
pub use output::WriteOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `write` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (parent must exist, atomic writes).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to the file to create or overwrite. Parent directories are automatically created if they do not exist."
            },
            "content": {
                "type": "string",
                "description": "Complete text content to write. Standard string with normal \\n line breaks."
            }
        },
        "required": ["path", "content"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "write".to_string(),
            description: "Creates a new file or fully overwrites an existing file with the provided content. \
                          Automatically creates parent directories if they do not exist. \
                          Pass `path` (absolute path) and `content` (full text with normal \\n line breaks). \
                          Returns a plain-text confirmation header."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "write".to_string(),
            description: "\
Creates a new file or fully overwrites an existing file with complete content.
Parent directories are automatically created if they do not exist.

## Parameters

- `path` (required): Absolute path to target file.
- `content` (required): Complete text content. Provide standard text with normal \\n line breaks.

## Output format

Returns a plain text confirmation header:
=== /path/to/file.txt (created, 128 bytes) ===
or
=== /path/to/file.txt (overwritten, 256 bytes) ==="
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the write tool.
///
/// Returns a `ToolResult` with either success (JSON WriteOutput) or failure (Text error message).
/// Returns `Err(WriteToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(WriteToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_fs_write::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({
///         "path": "/tmp/test.txt",
///         "content": "Hello, world!"
///     })
/// ).await.unwrap();
/// assert_eq!(result.name, "write");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, WriteToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Deserializes `args_json` and executes the write tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, WriteToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: WriteArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "write",
            Some(args.path.clone()),
            format!("Writing {}", args.path),
        ),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
