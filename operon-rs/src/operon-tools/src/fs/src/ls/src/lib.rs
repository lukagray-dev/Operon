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
use operon_tools_core::TieredToolDefinition;
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
                "description": "Absolute path to the directory to list."
            },
            "ignore": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Glob patterns matched against entry names to exclude. E.g. [\"*.lock\", \"node_modules\"]."
            }
        },
        "required": ["path"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "ls".to_string(),
            description: "Lists files and directories at a given path (single level, not recursive). \
                          Pass `path` (absolute directory path). Returns entries with type (FILE/DIR/SYMLINK), \
                          size, and last-modified time. Use `ignore` to exclude entries by name glob \
                          (e.g., [\"*.lock\", \"node_modules\", \".git\"]). Results capped at 1000 entries."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "ls".to_string(),
            description: "\
Lists files and directories at a given path (single level, not recursive).

## Input

`path` (required, string): Absolute path to the directory to list.
  - Must be a directory. Passing a file path returns an error result (not Err).
  - Must be an absolute path (relative paths are not supported).

`ignore` (optional, array of strings): Glob patterns to exclude entries by name.
  - Patterns are matched against the entry **name only**, not the full path.
  - Examples: [\"*.lock\", \"node_modules\", \".git\", \"target\", \".*\"]
  - If any pattern fails to compile, the entire listing fails with an error.
  - Default: empty (no exclusions).

## Output

Returns a JSON object with:
- `path`: The directory that was listed (echoed back for correlation).
- `entry_count`: Total number of entries in the listing (after exclusions).
- `truncated`: Boolean. True if more than 1000 entries exist (results are capped).
- `entries`: Array of directory entries, sorted: directories first (alphabetical), then files/symlinks (alphabetical).
- `error`: Human-readable error if the path could not be listed. When populated, entries is empty.

Each entry in `entries` contains:
- `name`: Entry name (not full path).
- `kind`: Entry type — one of: FILE, DIR, SYMLINK.
- `size_bytes`: File size in bytes (only for files; None for dirs/symlinks).
- `modified_unix`: Last modified timestamp as Unix seconds (None if unavailable).

## Behavior

- **Single-level only**: Does not recurse into subdirectories. Use grep or read for recursive operations.
- **Hidden files included**: Entries starting with '.' (e.g., .git, .env) ARE included by default.
  Use `ignore: [\".*\"]` to exclude them.
- **Symlinks**: Followed for metadata (size/modified time of the target). If the target is missing,
  metadata is None but the symlink is still listed as SYMLINK.
- **Metadata failures**: If metadata cannot be retrieved for an entry, the entry is still included
  but size_bytes and modified_unix are None.
- **Truncation**: If a directory contains more than 1000 entries, results are capped at 1000 and
  `truncated` is set to true. Increase the limit by calling the tool multiple times with different
  ignore patterns if needed.
- **Sorting**: Directories come first (alphabetical, case-insensitive), then files and symlinks
  (alphabetical, case-insensitive).

## Common mistakes

- Passing a file path instead of a directory → error result (not Err).
- Using relative paths → may fail or list unexpected directory.
- Expecting recursive output → use grep or read for recursive operations.
- Using `ignore` patterns that match full paths → patterns match entry names only.
  Use `ignore: [\"node_modules\"]` not `ignore: [\"**/node_modules\"]`.
- Forgetting to exclude hidden files → use `ignore: [\".*\"]` if needed.

## Examples

List a directory with no exclusions:
```json
{\"path\": \"/home/user/project\"}
```

List with exclusions:
```json
{\"path\": \"/home/user/project\", \"ignore\": [\"*.lock\", \"node_modules\", \".git\", \"target\"]}
```

Exclude hidden files:
```json
{\"path\": \"/home/user/project\", \"ignore\": [\".*\"]}
```"
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
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: LsArgs = serde_json::from_value(args_json)?;

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
