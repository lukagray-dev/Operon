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
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `read` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the single most important constraint (1 MB limit).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   edge cases, and worked examples.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Path to read a single file. Can include inline range suffix like 'src/main.rs:10-40', 'src/main.rs:5-EOF', or 'src/main.rs:15'."
            },
            "start_line": {
                "type": "integer",
                "description": "Optional start line for single file path (1-indexed, inclusive)."
            },
            "end_line": {
                "type": "integer",
                "description": "Optional end line for single file path (1-indexed, inclusive)."
            },
            "paths": {
                "type": "array",
                "description": "Files to read. Items can be path strings (with optional inline ranges like 'a.rs:10-40') or objects with path + start_line/end_line.",
                "items": {
                    "oneOf": [
                        {
                            "type": "string",
                            "description": "Path string, optionally with range suffix like 'src/main.rs:10-40', 'src/main.rs:5-EOF', or 'src/main.rs'."
                        },
                        {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Absolute or relative path to the file."
                                },
                                "start_line": {
                                    "type": "integer",
                                    "description": "First line to return (1-indexed, inclusive)."
                                },
                                "end_line": {
                                    "type": "integer",
                                    "description": "Last line to return (1-indexed, inclusive)."
                                }
                            },
                            "required": ["path"]
                        }
                    ]
                }
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "read".to_string(),
            description: "Reads one or multiple files (max 1 MB per file). \
                          Use `path` (or `paths`) with inline ranges like `\"src/main.rs:10-40\"` or `\"src/main.rs:5-EOF\"`. \
                          Returns raw file contents as plain text."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "read".to_string(),
            description: "\
Reads one or multiple files in a single call. Returns raw plain text with section headers.

## Input shapes

1. Inline string range (Recommended):
   `\"src/main.rs:10-40\"`   ← lines 10 to 40
   `\"src/main.rs:5-EOF\"`   ← line 5 to end of file
   `\"src/main.rs:15\"`      ← line 15 only
   `\"src/main.rs\"`         ← full file read

2. Root-level parameters:
   `{\"path\": \"src/main.rs\", \"start_line\": 10, \"end_line\": 40}`

3. Array of paths:
   `{\"paths\": [\"src/a.rs:10-40\", \"src/b.rs:5-EOF\", \"src/c.rs\"]}`

## Response format

Returns plain text with headers for each file:
=== src/main.rs (lines 10-40 of 200) ===
<raw content without line number prefixes>"
                .to_string(),
            parameters,
        },
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
