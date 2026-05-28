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
use serde_json::json;

/// Returns the ToolDefinition to register this tool with the model.
///
/// This definition describes the tool's name, purpose, and parameter schema
/// in a format that can be sent to any LLM provider via the normalization layer.
///
/// # Returns
/// A `ToolDefinition` that can be passed to `denormalize_definition` for
/// conversion to a provider-specific wire format.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "read".to_string(),
        description: "\
Reads one or multiple files in a single call. Returns file contents or per-file errors.

Each entry in `paths` can be:
- A plain string: `\"src/main.rs\"` — reads the entire file (max 1 MB)
- An object with optional line range: `{\"path\": \"src/main.rs\", \"start_line\": 100, \"end_line\": 200}`

Use line ranges to read large files in chunks. Binary files (images, executables) are not readable \
with this tool — use the dedicated media tool instead.

Always prefer reading multiple files in one call over sequential single-file calls."
            .to_string(),
        parameters: json!({
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
                                    "path": { "type": "string", "description": "Absolute or relative path to the file." },
                                    "start_line": { "type": "integer", "description": "First line to return (1-indexed, inclusive)." },
                                    "end_line": { "type": "integer", "description": "Last line to return (1-indexed, inclusive)." }
                                },
                                "required": ["path"]
                            }
                        ]
                    },
                    "minItems": 1
                }
            },
            "required": ["paths"]
        }),
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
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: ReadArgs = serde_json::from_value(args_json)?;
    
    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
