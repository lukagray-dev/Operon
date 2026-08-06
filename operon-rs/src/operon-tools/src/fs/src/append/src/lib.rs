//! # operon-tools-fs-append
//!
//! Implements the `append` tool for the Operon agent's filesystem group.
//!
//! Appends text to the end of an existing file without modifying existing content.
//! Supports:
//! - Appending to existing files (file must exist)
//! - Non-destructive operation (existing content is never modified or read)
//! - Atomic appends using OS-level append mode (O_APPEND)
//! - Validation that the file exists and is not a directory
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_fs_append::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "path": "/path/to/existing_file.txt",
//!     "content": "\nNew line to append"
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

pub use args::AppendArgs;
pub use error::AppendToolError;
pub use output::AppendOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `append` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (file must exist, non-destructive).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to an existing file to append to."
            },
            "content": {
                "type": "string",
                "description": "Text to append (standard string with \\n line breaks). Include leading \\n if a newline separator is needed."
            }
        },
        "required": ["path", "content"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "append".to_string(),
            description: "Appends text to the end of an existing file. Pass `path` (existing file path) \
                          and `content` (text to append with normal \\n line breaks; include leading \\n if separating newline is needed). \
                          The file must exist (use write for new files). Returns plain-text confirmation header."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "append".to_string(),
            description: "\
Appends text content to the end of an existing file.

## Parameters

- `path` (required): Absolute path to an existing file.
- `content` (required): Non-empty text content to append. Standard string with normal \\n line breaks. \
If a separating newline is needed before the new content, include it at the start of `content`.

## Output format

Returns a plain text confirmation header:
=== /path/to/file.txt (appended 64 bytes, total 512 bytes) ==="
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the append tool.
///
/// Returns a `ToolResult` with either success (JSON AppendOutput) or failure (Text error message).
/// Returns `Err(AppendToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(AppendToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_fs_append::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({
///         "path": "/tmp/test.txt",
///         "content": "\nNew line"
///     })
/// ).await.unwrap();
/// assert_eq!(result.name, "append");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, AppendToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Deserializes `args_json` and executes the append tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, AppendToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: AppendArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "append",
            Some(args.path.clone()),
            format!("Appending to {}", args.path),
        ),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
