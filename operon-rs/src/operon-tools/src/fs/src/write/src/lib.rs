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
                "description": "Absolute path to the file to create or overwrite. Parent directory must exist."
            },
            "content": {
                "type": "string",
                "description": "Complete file content. Existing content is fully replaced."
            }
        },
        "required": ["path", "content"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "write".to_string(),
            description: "Creates a new file or fully overwrites an existing file with the provided content. \
                          Pass `path` (absolute file path) and `content` (complete file content as a string). \
                          The parent directory must exist. This tool does not append or merge — existing content \
                          is completely replaced. Prefer the edit tool for partial changes to existing files."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "write".to_string(),
            description: "\
Creates a new file or fully overwrites an existing file with the provided content. All writes are atomic — \
if the write fails mid-operation, the original file (if it existed) is untouched.

## Input shapes

`path` (required, string): Absolute path to the file to create or overwrite. The parent directory must already exist. \
This tool does not create intermediate directories — if the parent doesn't exist, the tool returns an error.

`content` (required, string): Complete file content to write. For text files, this is the full UTF-8 string. \
Existing content is completely replaced — this tool does not append or merge.

## Worked examples

### Creating a new file
```json
{
  \"path\": \"/path/to/new_file.txt\",
  \"content\": \"Hello, world!\"
}
```

Result: A new file is created at `/path/to/new_file.txt` with content \"Hello, world!\".
The `created` field in the output is `true`.

### Overwriting an existing file
```json
{
  \"path\": \"/path/to/existing_file.txt\",
  \"content\": \"New content replaces everything\"
}
```

Result: The file at `/path/to/existing_file.txt` is completely replaced with the new content. \
The `created` field in the output is `false`.

## Parent directory must exist

This tool does not create intermediate directories. If the parent directory doesn't exist, the tool returns an error:
```
parent directory does not exist: /path/to/nonexistent/dir
```

To create the directory first, use the bash tool (or equivalent shell command).

## Atomic writes

The write is atomic: a temp file is created in the same directory, written to, then atomically renamed to the target path. \
If the write fails at any point (disk full, permission denied, etc.), the original file (if it existed) is NOT modified.

Error messages indicate whether the failure occurred during write or finalization:
- \"failed to write file: ...\" — temp file write failed. Original file untouched.
- \"failed to finalize write: ...\" — atomic rename failed. Original file untouched.

## Output fields

- `path`: The file path (echoed back for correlation).
- `created`: `true` if a new file was created, `false` if an existing file was overwritten.
- `bytes_written`: The number of bytes written (length of the content string).
- `message`: Human-readable summary (\"Created ...\" or \"Overwrote ...\").

## When to use write vs edit

Use `write` for:
- Creating new files
- Complete file rewrites where most or all content changes
- Replacing entire files with generated content

Use `edit` for:
- Partial changes to existing files (one function, one import, a few lines)
- Precise, targeted modifications
- When you want to preserve most of the file and only change specific regions

**Important**: Using `write` to make a small change to an existing file requires sending the entire file content, \
which is inefficient. Use `edit` instead — it only needs the changed region.

## Common mistakes

### Mistake #1: Parent directory doesn't exist
```json
{
  \"path\": \"/tmp/does_not_exist_xyz/file.txt\",
  \"content\": \"content\"
}
```

Error: \"parent directory does not exist: /tmp/does_not_exist_xyz\"

Fix: Create the directory first using the bash tool, then retry write.

### Mistake #2: Using write for a small change to an existing file
If you only need to change a few lines in a large file, use `edit` instead of `write`. \
`write` requires sending the entire file content; `edit` only needs the changed region.

### Mistake #3: Expecting write to append
This tool completely replaces the file content. It does not append. \
If you need to append, read the file first, concatenate the new content, then write the result.

## Error messages

- \"parent directory does not exist: ...\" → Create the directory first.
- \"failed to write file: ...\" → Disk full, permission denied, or other I/O error. File was not modified.
- \"failed to finalize write: ...\" → Atomic rename failed. File was not modified."
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
