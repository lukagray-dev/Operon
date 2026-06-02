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
                "description": "Text to append. Appended as-is — include a leading newline if needed."
            }
        },
        "required": ["path", "content"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "append".to_string(),
            description: "Appends text to the end of an existing file. Pass `path` (absolute path to an \
                          existing file) and `content` (text to append). The file must already exist — \
                          use the write tool to create new files. Never modifies existing content. \
                          Does not require reading the file first."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "append".to_string(),
            description: "\
Appends text to the end of an existing file without modifying or reading existing content. \
The file must already exist — if it doesn't, the tool returns an error. Use the write tool to create new files.

## Input shapes

`path` (required, string): Absolute path to an existing file to append to. The file must already exist. \
If the file does not exist, the tool returns an error and nothing is appended. If the path is a directory, \
the tool returns an error.

`content` (required, string): Text to append to the end of the file. Appended as-is — no automatic newline \
insertion. If a newline separator is needed between the existing content and the new content, include it \
at the start of this string. The content must be non-empty — appending empty content is an error.

## Worked examples

### Appending a new line to a log file
```json
{
  \"path\": \"/var/log/app.log\",
  \"content\": \"[2025-05-29] New log entry\\n\"
}
```

Result: The log entry is appended to the end of the file. Existing log entries are untouched.

### Appending to a file without a trailing newline
```json
{
  \"path\": \"/path/to/file.txt\",
  \"content\": \"\\nNew line\"
}
```

Result: A newline is inserted first (from the content string), then \"New line\" is appended. \
This ensures the new content starts on a fresh line.

### Appending without a leading newline (concatenation)
```json
{
  \"path\": \"/path/to/file.txt\",
  \"content\": \"more text\"
}
```

Result: \"more text\" is appended directly to the end of the file, even if the file doesn't end with a newline. \
The appended text will run onto the last line of the file.

## File must exist

This tool does not create files. If the file doesn't exist, the tool returns an error:
```
file does not exist: /path/to/file.txt. Use the write tool to create new files.
```

To create the file first, use the write tool.

## Non-destructive operation

The append tool is non-destructive — existing content is never modified or read. Unlike the write tool \
(which requires sending the entire file content) or the edit tool (which requires reading the file first), \
append only needs the new content. This makes it ideal for:
- Adding entries to logs or accumulating output
- Appending to configuration files
- Building up file content incrementally

## Output fields

- `path`: The file path (echoed back for correlation).
- `bytes_appended`: The number of bytes appended (length of the content string in UTF-8).
- `total_bytes`: The total file size in bytes after the append.
- `message`: Human-readable summary (\"Appended N bytes to path/to/file.ext (total: M bytes)\").

## When to use append vs write vs edit

Use `append` for:
- Adding new lines to an existing file (logs, accumulating output, adding entries to a list)
- Cheapest operation — no read needed, no temp file needed
- Building up file content incrementally

Use `write` for:
- Creating a new file
- Replacing entire file content
- When you need to send the complete file content

Use `edit` for:
- Modifying specific lines within a file
- Precise, targeted changes to existing content
- When you want to preserve most of the file and only change specific regions

**Important**: Using `write` to make a small change to an existing file requires sending the entire file content, \
which is inefficient. Use `edit` instead — it only needs the changed region. Using `append` is even cheaper \
if you're just adding to the end.

## Common mistakes

### Mistake #1: File doesn't exist
```json
{
  \"path\": \"/tmp/does_not_exist_xyz/file.txt\",
  \"content\": \"content\"
}
```

Error: \"file does not exist: /tmp/does_not_exist_xyz/file.txt. Use the write tool to create new files.\"

Fix: Use the write tool to create the file first, then append to it.

### Mistake #2: Forgetting the leading newline
If the existing file doesn't end with a newline, and you want the appended content on a new line, \
include a leading newline in the content:

```json
{
  \"path\": \"/path/to/file.txt\",
  \"content\": \"\\nNew line\"
}
```

Without the leading newline, the appended text will run onto the last line of the file.

### Mistake #3: Appending empty content
```json
{
  \"path\": \"/path/to/file.txt\",
  \"content\": \"\"
}
```

Error: \"content is empty — nothing to append\"

Fix: Only append non-empty content.

### Mistake #4: Trying to append to a directory
```json
{
  \"path\": \"/path/to/directory\",
  \"content\": \"content\"
}
```

Error: \"path is a directory, not a file: /path/to/directory\"

Fix: Ensure the path points to a file, not a directory.

## Error messages

- \"file does not exist: ...\" → Use the write tool to create the file first.
- \"path is a directory, not a file: ...\" → Ensure the path points to a file.
- \"content is empty — nothing to append\" → Only append non-empty content.
- \"failed to append to file: ...\" → I/O error (permission denied, disk full, etc.). File was not modified."
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
