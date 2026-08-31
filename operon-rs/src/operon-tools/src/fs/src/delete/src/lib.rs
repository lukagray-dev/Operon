//! # operon-tools-fs-delete
//!
//! Implements the `delete` tool for the Operon agent's filesystem group.
//!
//! Deletes a file or directory, with two modes:
//! - **Trash mode (default)**: Moves the target to the system trash (macOS Trash, Windows Recycle Bin,
//!   Linux trash-spec). The file is recoverable by the user from their system trash.
//! - **Permanent mode**: Permanently deletes the target with no recovery possible. Irreversible.
//!
//! Supports:
//! - Deleting individual files
//! - Deleting entire directory trees (recursive)
//! - Deleting symlinks (the symlink itself is deleted, not the target)
//! - Safe defaults (trash by default, not permanent)
//! - Validation that the path exists before deletion
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_fs_delete::{definition, execute};
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
//!     "permanent": false  // or omit for default (false)
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

pub use args::DeleteArgs;
pub use error::DeleteToolError;
pub use output::{DeleteOutput, DeletedKind};

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use serde_json::json;

/// Returns the canonical tool definition for the `delete` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`path`).
/// - Clear documentation for `path` and `permanent` deletion mode.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the parameters schema for deleting files or directories here.
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "File or directory path to delete."
            },
            "permanent": {
                "type": "boolean",
                "default": false,
                "description": "If false (default), move to system trash (recoverable). If true, permanently delete. Prefer false."
            }
        },
        "required": ["path"]
    });

    ToolDefinition {
        name: "delete".to_string(),
        description: "Deletes a file or directory. Pass `path` (file or directory path) and optionally \
                      `permanent` (bool, default false). When permanent is false (default), the \
                      target is moved to the system trash and can be recovered. When permanent \
                      is true, it is deleted with no recovery possible. Prefer permanent: false \
                      unless permanent deletion is explicitly required."
            .to_string(),
        parameters,
    }
}

/// Deserializes `args_json` and executes the delete tool.
///
/// Returns a `ToolResult` with either success (JSON DeleteOutput) or failure (Text error message).
/// Returns `Err(DeleteToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(DeleteToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_fs_delete::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({
///         "path": "/tmp/file.txt",
///         "permanent": false
///     })
/// ).await.unwrap();
/// assert_eq!(result.name, "delete");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, DeleteToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Deserializes `args_json` and executes the delete tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, DeleteToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: DeleteArgs = serde_json::from_value(args_json)?;

    let message = if args.permanent {
        format!("Permanently deleting {}", args.path)
    } else {
        format!("Moving {} to trash", args.path)
    };

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "delete", Some(args.path.clone()), message),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
