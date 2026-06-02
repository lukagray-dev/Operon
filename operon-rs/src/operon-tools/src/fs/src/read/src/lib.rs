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
            "paths": {
                "type": "array",
                "description": "Files to read. Each item is either a path string or an object with path + optional start_line/end_line.",
                "items": {
                    "oneOf": [
                        {
                            "type": "string",
                            "description": "Absolute or relative path to the file."
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
                },
                "minItems": 1
            }
        },
        "required": ["paths"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "read".to_string(),
            description: "Reads one or multiple files in one call (max 1 MB per file). \
                          Pass `paths` as an array of strings or objects. \
                          Use `{\"path\": \"...\", \"start_line\": N, \"end_line\": M}` to read a \
                          line range instead of the full file. \
                          Binary files cannot be read with this tool."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "read".to_string(),
            description: "\
Reads one or multiple files in a single call. Returns structured per-file results.

## Input shapes

`paths` is a required array. Each element is ONE of:

1. A plain string — reads the entire file (subject to 1 MB limit):
   `\"src/main.rs\"`

2. An object — reads the entire file or a line range:
   `{\"path\": \"src/main.rs\"}`
   `{\"path\": \"src/main.rs\", \"start_line\": 100, \"end_line\": 200}`
   `{\"path\": \"src/main.rs\", \"start_line\": 50}`   ← reads from line 50 to EOF
   `{\"path\": \"src/main.rs\", \"end_line\": 30}`     ← reads from line 1 to line 30

You can mix both shapes in the same call:
`{\"paths\": [\"Cargo.toml\", {\"path\": \"src/main.rs\", \"start_line\": 1, \"end_line\": 50}]}`

## Size limit

Full-file reads (no line range) are capped at 1 MB. If a file exceeds this,
the result for that file will have `success: false` with an error message telling
you to use `start_line`/`end_line`. Line-range reads bypass the size check.

## Response format

Returns a JSON object: `{\"files\": [ ...one entry per path... ]}`

Each entry:
- `success: true`  → `content` (string), `total_lines` (int), optionally `lines_returned`
- `success: false` → `error` (string describing why it failed)

The overall tool call always returns `is_error: false`. Per-file failures are inside the JSON.

## Constraints

- Binary files (null bytes) → `success: false`, error message.
- Invalid UTF-8 → `success: false`, error message.
- Non-existent path → `success: false`, error message.
- `start_line` beyond file end → `success: false`, error message.
- `end_line` beyond file end → clamped silently to last line, no error.

## Common mistakes

- Passing `paths` as a string instead of an array → args parse failure.
- Using `path` as the top-level key instead of `paths` → args parse failure.
- Passing line numbers as strings instead of integers → args parse failure."
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
            format!("Reading {} file(s)", args.paths.len()),
        ),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
