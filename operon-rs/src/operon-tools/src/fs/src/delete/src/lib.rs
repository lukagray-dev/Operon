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
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `delete` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (path must exist, two deletion modes).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, common mistakes, and strong safety guidance.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to the file or directory to delete."
            },
            "permanent": {
                "type": "boolean",
                "default": false,
                "description": "If false (default), move to system trash (recoverable). If true, permanently delete. Prefer false."
            }
        },
        "required": ["path"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "delete".to_string(),
            description: "Deletes a file or directory. Pass `path` (absolute path) and optionally \
                          `permanent` (bool, default false). When permanent is false (default), the \
                          target is moved to the system trash and can be recovered. When permanent \
                          is true, it is deleted with no recovery possible. Prefer permanent: false \
                          unless permanent deletion is explicitly required."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "delete".to_string(),
            description: "\
Deletes a file or directory. Pass `path` (absolute path to an existing file or directory) and optionally \
`permanent` (bool, default false). The path must exist — if it doesn't, the tool returns an error.

## Input shapes

`path` (required, string): Absolute path to the file or directory to delete. The path must exist. \
If the path does not exist, the tool returns an error and nothing is deleted. Both files and directories \
are supported. For directories, the entire tree is deleted (all contents recursively).

`permanent` (optional, bool, default false): Controls the deletion mode:
- `false` (default): Move the target to the system trash. The file is recoverable by the user from their \
  system trash (macOS Trash, Windows Recycle Bin, Linux trash-spec). Use this for almost all deletions.
- `true`: Permanently delete the target with no recovery possible. Irreversible. Use only when permanent \
  removal is explicitly required (e.g., deleting a temp file that must not persist in trash, removing \
  secrets from disk).

## Deletion modes in detail

### Trash mode (permanent: false, default)
The target is moved to the system trash. The file is NOT deleted from disk — it is moved to a special \
location where the user can recover it. This is the safe default.

- macOS: Moved to ~/Trash
- Windows: Moved to Recycle Bin
- Linux: Moved to ~/.local/share/Trash (trash-spec)

Use this for almost all deletions. The user can recover the file if needed.

### Permanent mode (permanent: true)
The target is permanently deleted using `remove_file` (for files) or `remove_dir_all` (for directories). \
This is irreversible — if the wrong path is deleted, the data is unrecoverable.

Use only when permanent removal is explicitly required:
- Deleting a temp file that must not persist in trash
- Removing secrets or sensitive data from disk
- Cleaning up after a build or test run

**WARNING**: Permanent deletion is irreversible. If the wrong path is deleted, the data is unrecoverable. \
Always double-check the path before using permanent: true.

## Files and directories

Both files and directories are supported:
- **Files**: The file is deleted (or moved to trash).
- **Directories**: The entire directory tree is deleted recursively (or moved to trash). All contents \
  (subdirectories, files) are deleted along with the directory itself.
- **Symlinks**: The symlink itself is deleted, not the target. The target remains untouched.

## Output fields

- `path`: The path that was deleted (echoed back for correlation).
- `kind`: Either \"file\" or \"dir\" — indicates what was deleted.
- `permanent`: Whether the deletion was permanent (true) or moved to trash (false).
- `message`: Human-readable summary (\"Moved {path} to trash (file|dir)\" or \
  \"Permanently deleted {path} (file|dir)\").

## When to use delete vs other tools

Use `delete` for:
- Removing files or directories that are no longer needed
- Cleaning up temporary files
- Removing secrets or sensitive data (with permanent: true)

Use `write` for:
- Creating new files
- Replacing entire file content

Use `edit` for:
- Modifying specific lines within a file

## Worked examples

### Delete a file to trash (safe, recoverable)
```json
{
  \"path\": \"/tmp/temp_file.txt\",
  \"permanent\": false
}
```

Result: The file is moved to the system trash. The user can recover it from their trash.

### Delete a file permanently (irreversible)
```json
{
  \"path\": \"/tmp/secret.key\",
  \"permanent\": true
}
```

Result: The file is permanently deleted. No recovery is possible.

### Delete a directory and all its contents to trash
```json
{
  \"path\": \"/tmp/build_output\",
  \"permanent\": false
}
```

Result: The directory and all its contents are moved to trash. Recoverable.

### Delete a directory permanently
```json
{
  \"path\": \"/tmp/cache\",
  \"permanent\": true
}
```

Result: The directory and all its contents are permanently deleted. No recovery is possible.

### Omit permanent (defaults to false)
```json
{
  \"path\": \"/tmp/file.txt\"
}
```

Result: The file is moved to trash (permanent defaults to false). Safe default.

## Common mistakes

### Mistake #1: Path doesn't exist
```json
{
  \"path\": \"/tmp/does_not_exist_xyz/file.txt\",
  \"permanent\": false
}
```

Error: \"path does not exist: /tmp/does_not_exist_xyz/file.txt\"

Fix: Verify the path exists before calling delete.

### Mistake #2: Using permanent: true when permanent: false would suffice
```json
{
  \"path\": \"/tmp/file.txt\",
  \"permanent\": true
}
```

This permanently deletes the file. If the wrong path was specified, the data is unrecoverable.

Fix: Always prefer permanent: false (or omit it). Only use permanent: true when you have a specific reason.

### Mistake #3: Trying to delete a non-existent nested path
```json
{
  \"path\": \"/tmp/does_not_exist_xyz_operon/subdir/file.txt\",
  \"permanent\": false
}
```

Error: \"path does not exist: /tmp/does_not_exist_xyz_operon/subdir/file.txt\"

Fix: Verify the entire path exists before calling delete.

### Mistake #4: Forgetting that permanent deletion is irreversible
Using permanent: true on the wrong path will permanently delete the data with no recovery possible. \
Always verify the path is correct before using permanent: true.

## Error messages

- \"path does not exist: ...\" → Verify the path exists before calling delete.
- \"failed to access path: ...\" → Permission denied or other I/O error. Check permissions.
- \"failed to move to trash: ...\" → Trash operation failed (disk full, permission denied, etc.).
- \"failed to delete file: ...\" → Permanent deletion failed (permission denied, etc.).
- \"failed to delete directory: ...\" → Permanent deletion of directory failed (permission denied, etc.).
- \"internal error: delete task panicked\" → Internal error. This should not happen — report if seen.

## Safety guidance

**Prefer permanent: false for all deletions.** Permanent deletion is irreversible — if the wrong path is \
deleted, the data is unrecoverable. Only use permanent: true when you have a specific reason (temp files, \
secrets that must not remain in trash).

Always verify the path is correct before calling delete, especially with permanent: true."
                .to_string(),
            parameters,
        },
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
