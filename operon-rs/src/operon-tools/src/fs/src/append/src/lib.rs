//! # operon-tools-fs-append
//!
//! Implements the `append` tool for the Operon agent's filesystem group.
//!
//! Appends text to the end of an existing file without modifying existing content.
//! Supports:
//! - Appending to existing files (file must exist — use write to create)
//! - Non-destructive operation (existing content is never modified or read)
//! - Atomic appends using OS-level append mode (O_APPEND)
//! - Validation that the file exists and is not a directory
//!
//! ## Call format
//!
//! ```
//! <append path="C:\absolute\path\to\file.txt">
//! <<<<
//! content to append
//! with real line breaks
//! >>>>
//! ```
//!
//! The dispatcher injects:
//! - `args_json["path"]`     — the absolute file path from the `path` attr.
//! - `args_json["__body__"]` — the raw body content between the tag and `>>>>`.
//!
//! ## Output format (plain text)
//!
//! - Success:    `"{path} done"`
//! - Any error:  `"{path}\nERROR: {reason}"`

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

// Export the Args and Error types. AppendOutput no longer exists.
pub use args::AppendArgs;
pub use error::AppendToolError;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `append` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints.
/// - `detailed`: sent after a malformed call. Full explanation with call format,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    // The schema only declares "path" — body content arrives via __body__,
    // which is injected by the dispatcher and is not part of the JSON schema.
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to an existing file to append to."
            }
        },
        "required": ["path"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "append".to_string(),
            description: "Appends text to the end of an existing file. path attr is the \
                          absolute file path. Write content to append as raw text in the \
                          tool body. File must exist."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "append".to_string(),
            description: "\
Appends text to the end of an existing file without modifying or reading existing content. \
The file must already exist — if it doesn't, the tool returns an error. Use the write tool \
to create new files.

## Call format

```
<append path=\"/path/to/existing_file.txt\">
<<<<

content to append
with real line breaks
>>>>
```

The `path` attr is the absolute path to an existing file. \
The body between the opening tag and `>>>>` is the content to append, verbatim.

## Output format

- Success:   `\"{path} done\"`
- Any error: `\"{path}\\nERROR: {reason}\"`

## Worked examples

### Appending a new line to a log file

```
<append path=\"/var/log/app.log\">
<<<<
[2025-05-29] New log entry
>>>>
```

Result: `/var/log/app.log done`

### Appending without a leading newline (concatenation)

```
<append path=\"/path/to/file.txt\">
<<<<
more text
>>>>
```

Result: \"more text\" is appended directly. If a newline separator is needed \
before the new content, include it at the start of the body.

## File must exist

This tool does not create files. If the file doesn't exist:
```
/path/to/file.txt
ERROR: file does not exist
```

Use the write tool to create the file first, then append to it.

## Non-destructive operation

The append tool is non-destructive — existing content is never modified or read. \
Unlike write (which requires sending the entire file content), append only needs \
the new content. Ideal for:
- Adding entries to logs or accumulating output
- Appending to configuration files
- Building up file content incrementally

## Error messages

- `ERROR: content is empty` → Body was empty; only append non-empty content.
- `ERROR: file does not exist` → Use write to create the file first.
- `ERROR: path is a directory` → Ensure the path points to a file, not a directory.
- `ERROR: ...` → I/O error (permission denied, disk full, etc.)."
                .to_string(),
            parameters,
        },
    }
}

/// Parses `args_json` and executes the append tool.
///
/// Returns a `ToolResult` with ToolContent::Text for both success and error.
/// Returns `Err(AppendToolError::ArgsParse)` only if the required `path` attr is
/// missing or malformed — all other errors are returned as Ok(ToolResult).
///
/// # Arguments
/// - `call_id`:   The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments injected by the dispatcher. Must contain
///                `"path"` (String) and optionally `"__body__"` (String).
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, AppendToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the append tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, AppendToolError> {
    // Parse arguments manually (no serde Deserialize).
    // A missing or empty "path" is the only hard failure — body absence is allowed.
    let args = AppendArgs::parse(&args_json).map_err(AppendToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "append",
            Some(args.path.clone()),
            format!("Appending to {}", args.path),
        ),
    );

    // Execute and return the result. The executor always returns Ok(ToolResult).
    Ok(executor::execute(call_id, args).await)
}
