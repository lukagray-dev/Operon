//! # operon-tools-fs-read
//!
//! Implements the `read` tool for the Operon agent's filesystem group.
//!
//! Reads one or multiple files in a single tool call. Supports:
//! - Multi-file batched reads (concurrent)
//! - Chunked reading via start_line/end_line for large files
//! - Binary file detection
//! - 1 MB size limit on full-file reads
//! - Per-file success/error in a single structured ToolResult
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_fs_read::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "paths": ["src/main.rs", "Cargo.toml"]
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
pub use output::{FileReadResult, LineRange, ReadOutput};

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use serde_json::json;

/// Returns the canonical tool definition for the `read` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Clear, concise description of capabilities, inline line range syntax, and batching recommendations.
/// - Descriptive JSON schema properties for `paths` and `path`.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the JSON Schema parameters for the read tool here.
    // We explain the arguments clearly so models know they can read single files
    // or batch multiple files/line ranges in one single call!
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": ["string", "array"],
                "items": {
                    "type": "string",
                    "description": "File path string with optional inline range suffix like 'src/main.rs:10-40', 'src/main.rs:5-EOF', or 'src/main.rs'."
                },
                "description": "File path to read, or an array of file paths to read in batch. Always pass multiple paths in an array (e.g. ['src/a.rs', 'src/b.rs:10-50']) to read all files in a single tool call rather than issuing multiple sequential read calls. Supports optional :start-end line ranges."
            }
        },
        "required": ["path"]
    });

    ToolDefinition {
        name: "read".to_string(),
        description: "Reads one or multiple files in a single call (max 1 MB per file). \
                      Pass `path` as a single path string or an array of path strings. \
                      Always prefer batching multiple files into an array (e.g. `path: [\"src/a.rs:10-40\", \"src/b.rs\"]`) in ONE tool call instead of calling `read` multiple times sequentially. \
                      Supports inline line ranges like `:10-40` or `:5-EOF`. Returns raw plain text with per-file headers."
            .to_string(),
        parameters,
    }
}

/// Deserializes `args_json` and executes the read tool.
///
/// Returns a `ToolResult` with `is_error: false` even on partial file failures —
/// per-file errors are embedded in the JSON content.
/// Returns `Err(ReadToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with per-file results in JSON content.
/// - `Err(ReadToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_fs_read::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({ "paths": ["Cargo.toml"] })
/// ).await.unwrap();
/// assert_eq!(result.name, "read");
/// assert!(!result.is_error);
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, ReadToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Deserializes `args_json` and executes the read tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, ReadToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: ReadArgs = serde_json::from_value(args_json)?;
    if args.path.is_none() {
        return Err(ReadToolError::ArgsParse(serde::de::Error::custom(
            "must provide 'path'",
        )));
    }

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "read",
            None,
            format!("Reading {} file(s)", args.target_count()),
        ),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
