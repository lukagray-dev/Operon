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
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use serde_json::json;

/// Returns the canonical tool definition for the `grep` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`pattern`).
/// - Comprehensive parameter documentation for pattern, paths, globs, case insensitivity, and context lines.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the JSON Schema parameters for the grep search tool here.
    let parameters = json!({
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "Regex pattern to search for (Rust regex syntax)."
            },
            "path": {
                "type": ["string", "array"],
                "items": { "type": "string" },
                "description": "File or directory path to search, or an array of paths to search in batch. Always pass multiple paths in an array (e.g. ['src', 'tests']) to search across multiple locations in a single tool call."
            },
            "include": {
                "type": "string",
                "description": "Optional glob pattern to filter files by name (e.g. \"*.rs\", \"*.{ts,tsx}\")."
            },
            "case_insensitive": {
                "type": "boolean",
                "description": "Case-insensitive matching. Default: false."
            },
            "context_lines": {
                "type": "integer",
                "minimum": 0,
                "description": "Number of context lines before and after each match. Default: 2."
            }
        },
        "required": ["pattern"]
    });

    ToolDefinition {
        name: "grep".to_string(),
        description: "Searches files and directories for a regex pattern. \
                      Pass `pattern` (regex string) and `path` (file/directory path or array of paths). \
                      Always prefer batching multiple search paths into an array (e.g. `path: [\"src\", \"tests\"]`) in ONE tool call rather than issuing multiple sequential grep calls. \
                      Directories are walked recursively respecting .gitignore. \
                      Use `include` to filter by filename glob (e.g., \"*.rs\")."
            .to_string(),
        parameters,
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
