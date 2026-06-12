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
//! - Plain-text output (no JSON)
//!
//! ## Call format
//!
//! ```text
//! <!-- Simple (trash, default): -->
//! <delete path="C:\absolute\path\to\file.txt">
//!
//! <!-- Permanent: -->
//! <delete path="C:\absolute\path\to\file.txt">
//! <<<<
//! permanent="true"
//! >>>>
//! ```

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::DeleteArgs;
pub use error::DeleteToolError;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `delete` tool.
///
/// - `short`: sent to the model under normal conditions. Concise.
/// - `detailed`: sent after a malformed call. Full explanation with body format,
///   error cases, and safety guidance.
pub fn definition() -> TieredToolDefinition {
    // Schema: only `path` is an attribute. `permanent` lives in the body.
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to the file or directory to delete."
            }
        },
        "required": ["path"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "delete".to_string(),
            description: "Deletes a file or directory. path attr is the absolute path. By default \
                          moves to system trash (recoverable). Add permanent=\"true\" in the tool \
                          body to permanently delete with no recovery possible."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "delete".to_string(),
            description: "\
Deletes a file or directory. The path must exist. Returns plain-text output.

## Call format

Simple (trash, default — recoverable):
  <delete path=\"C:\\absolute\\path\\to\\file.txt\">

Permanent (irreversible — no recovery):
  <delete path=\"C:\\absolute\\path\\to\\file.txt\">
  <<<<
  permanent=\"true\"
  >>>>

## Body options

- `permanent`: \"true\" = permanently delete (irreversible). \"false\" or omitted = move to system trash.

## Deletion modes

### Trash mode (default)
Moves the target to the system trash. The file is NOT deleted from disk — moved to a special
location where the user can recover it.
- macOS: ~/Trash
- Windows: Recycle Bin
- Linux: ~/.local/share/Trash

### Permanent mode (permanent=\"true\")
Permanently deletes using remove_file (files) or remove_dir_all (directories).
Irreversible. Use only when explicitly required.

## Output format

Success:
  C:\\path\\to\\file.txt permanently deleted (file)
  C:\\path\\to\\dir permanently deleted (dir)
  C:\\path\\to\\file.txt moved to trash (file)
  C:\\path\\to\\dir moved to trash (dir)

Errors (all inline, is_error: false):
  path does not exist: {path}
  failed to access path: {path}: {reason}
  failed to move to trash: {reason}
  failed to delete file: {reason}
  failed to delete directory: {reason}
  internal error: delete task panicked

## Files and directories

- **Files**: The file is deleted (or moved to trash).
- **Directories**: The entire directory tree is deleted recursively.
- **Symlinks**: The symlink itself is deleted, not the target.

## Safety guidance

Prefer permanent=\"false\" (or omit) for all deletions. Permanent deletion is irreversible.
Always verify the path is correct before using permanent=\"true\".

## Common mistakes

- Path doesn't exist → verify path before calling delete.
- Using permanent=\"true\" when trash is sufficient → irreversible data loss risk.
- Passing `permanent` as an attribute instead of in the body."
                .to_string(),
            parameters,
        },
    }
}

/// Parses `args_json` and executes the delete tool.
///
/// Returns a `ToolResult` with `is_error: false` always — errors are embedded
/// inline in the plain-text output.
/// Returns `Err(DeleteToolError::ArgsParse)` only if the `path` attribute is
/// missing or the body contains an invalid value.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments sent by the parser.
///
/// # Returns
/// - `Ok(ToolResult)` with the deletion result as plain text.
/// - `Err(DeleteToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, DeleteToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the delete tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, DeleteToolError> {
    // Parse the arguments from path attr + body.
    let args = DeleteArgs::parse(&args_json).map_err(DeleteToolError::ArgsParse)?;

    // Emit progress so the UI shows "Moving/Permanently deleting {path}" while waiting.
    let message = if args.permanent {
        format!("Permanently deleting {}", args.path)
    } else {
        format!("Moving {} to trash", args.path)
    };

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "delete", Some(args.path.clone()), message),
    );

    // Execute the deletion (always Ok — errors are inline in the text output).
    Ok(executor::execute(call_id, args).await)
}
