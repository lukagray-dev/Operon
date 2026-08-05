//! # operon-tools-fs-grep
//!
//! Implements the `grep` tool for the Operon agent's filesystem group.
//!
//! Searches files and directories for regex patterns. Supports:
//! - Regex pattern matching with case-sensitive/insensitive modes
//! - Recursive directory walking with gitignore rules respected
//! - Filename glob filtering (e.g., "*.rs" to search only Rust files)
//! - Context lines before/after matches
//! - Per-file match reporting with line numbers
//! - 300 match limit to prevent context overflow
//! - 10 MB file size limit
//! - Binary file detection and skipping
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_fs_grep::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "pattern": "fn main",
//!     "paths": ["src/"],
//!     "include": "*.rs"
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

pub use args::GrepArgs;
pub use error::GrepToolError;
pub use output::{FileGrepResult, GrepLine, GrepOutput};

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `grep` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the key constraints (300 match limit, 10 MB file limit).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   edge cases, common mistakes, and worked examples.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "Regex pattern to search for. Uses Rust regex syntax."
            },
            "path": {
                "type": "string",
                "description": "Single file or directory path to search."
            },
            "paths": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Files or directories to search."
            },
            "include": {
                "type": "string",
                "description": "Optional glob pattern to filter files by name (e.g., \"*.rs\", \"*.{ts,tsx}\")."
            },
            "case_insensitive": {
                "type": "boolean",
                "description": "Case-insensitive matching. Default: false."
            },
            "context_lines": {
                "type": "integer",
                "minimum": 0,
                "description": "Number of context lines before/after each match. Default: 2."
            }
        },
        "required": ["pattern"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "grep".to_string(),
            description: "Searches files and directories for a regex pattern. \
                          Pass `pattern` (regex string) and `path` or `paths` (file/dir paths). \
                          Directories are walked recursively respecting .gitignore. \
                          Use `include` to filter by filename glob (e.g., \"*.rs\")."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "grep".to_string(),
            description: "\
Searches files and directories recursively for a regex pattern. Returns plain text with line numbers.

## Input shapes

1. Single path or array of paths:
   `{\"pattern\": \"fn main\", \"path\": \"src\"}`
   `{\"pattern\": \"TODO\", \"paths\": [\"src\", \"tests\"]}`

2. With optional filters & context:
   `{\"pattern\": \"error\", \"path\": \"src\", \"include\": \"*.rs\", \"case_insensitive\": true, \"context_lines\": 2}`

## Response format

Returns plain text grouped by file:
=== src/main.rs (2 matches) ===
10: fn main() {
11:     println!(\"Hello\");
---
45: fn run() {

Showing 2 match(es) across 1 file(s)."
                .to_string(),
            parameters,
        },
    }
}


/// Deserializes `args_json` and executes the grep tool.
///
/// Returns a `ToolResult` with `is_error: false` for successful searches (even if no matches found).
/// Returns `is_error: true` only for invalid regex patterns or internal serialization bugs.
/// Per-file errors (permission denied, binary files, etc.) are embedded in the JSON content.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with per-file results in JSON content.
/// - `Err(GrepToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_fs_grep::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({ "pattern": "TODO", "paths": ["src/"] })
/// ).await.unwrap();
/// assert_eq!(result.name, "grep");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, GrepToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Deserializes `args_json` and executes the grep tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, GrepToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: GrepArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "grep",
            None,
            format!("Searching {} path(s)", args.get_paths().len()),
        ),
    );


    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics), so we can directly return it.
    Ok(executor::execute(call_id, args).await)
}
