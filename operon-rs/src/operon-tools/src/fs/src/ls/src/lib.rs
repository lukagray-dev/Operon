//! # operon-tools-fs-ls
//!
//! Implements the `ls` tool for the Operon agent's filesystem group.
//!
//! Lists files and directories at a given path. Supports:
//! - Recursive depth control (depth=1 default, depth=0 for unlimited)
//! - File name glob filtering (e.g., "*.py" lists only Python files)
//! - Entry ignore patterns (skip entries by name)
//! - 1000 entry limit to prevent overwhelming the model
//! - Human-readable file sizes
//! - Plain-text output with [DIR] and [FILE] prefixes
//!
//! ## Call format
//!
//! ```text
//! <!-- Simple: list immediate children -->
//! <ls path="C:\absolute\path\to\directory">
//!
//! <!-- With options: -->
//! <ls path="C:\absolute\path\to\directory">
//! <<<<
//! depth="2"
//! glob="*.py"
//! ignore="node_modules" ".git"
//! >>>>
//! ```

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::LsArgs;
pub use error::LsToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `ls` tool.
///
/// - `short`: sent to the model under normal conditions. Concise.
/// - `detailed`: sent after a malformed call. Full body format explanation.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "ls".to_string(),
            description: "Lists files and directories. path attr is the root directory. Optionally \
                          write options in the tool body: depth=\"2\" (default 1, 0=unlimited), \
                          glob=\"*.py\" to filter files, ignore=\"node_modules\" to skip entries. \
                          Output includes sizes. Capped at 1000 entries."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "ls".to_string(),
            description: "\
Lists files and directories at a given path. Returns plain-text output.

## Call format

The `path` attribute is the root directory to list. All options go in the tool body.

Simple (lists immediate children):
  <ls path=\"C:\\absolute\\path\\to\\directory\">

With options:
  <ls path=\"C:\\absolute\\path\\to\\directory\">
  <<<<
  depth=\"2\"
  glob=\"*.py\"
  ignore=\"node_modules\" \".git\"
  >>>>

## Body options

- `depth`: Tree depth. 1 = single level (default). 0 = unlimited recursion.
           depth=2 means immediate children and their immediate children.
- `glob`: Glob pattern to filter file names (not applied to directory names).
          Only the first token is used. E.g. \"*.rs\", \"*.{ts,tsx}\".
- `ignore`: Entry names to skip. Each token is a separate pattern.
            Applied to both files and directories — matching dirs are also excluded from recursion.
            E.g. ignore=\"node_modules\" \".git\" \"target\"

## Output format

```
C:\\absolute\\path\\to\\directory
[DIR]  src
[DIR]  src/utils
[FILE] src/utils/math.py (4.2 KB)
[FILE] src/utils/helpers.py (9.3 KB)
[DIR]  src/api
[FILE] src/api/orders.py (2.1 KB)
```

- Paths are relative to the root path argument (using forward slashes).
- Directories come before files at each level (alphabetical, case-insensitive).
- Files are also sorted alphabetically (case-insensitive).
- File sizes shown as human-readable (B, KB, MB). Directories show no size.
- Header is the root path.
- Capped at 1000 entries. \"***omitted N entries***\" appended if truncated.

## Error format

If the path doesn't exist or is a file:
  {path}
  ERROR: {reason}

## Constraints

- Non-existent path → ERROR inline.
- File path instead of directory → ERROR inline.
- Unreadable subdirectories are silently skipped during recursion.
- Non-UTF-8 entry names are silently skipped.

## Common mistakes

- Passing a file path instead of a directory.
- Using relative paths — always use absolute paths.
- Expecting recursive output without setting depth > 1."
                .to_string(),
        },
    }
}

/// Parses `args_json` and executes the ls tool.
///
/// Returns a `ToolResult` with `is_error: false` always — directory errors are
/// embedded inline in the text output.
/// Returns `Err(LsToolError::ArgsParse)` only if the `path` attribute is missing
/// or the body contains an invalid value.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments sent by the parser.
///
/// # Returns
/// - `Ok(ToolResult)` with directory listing as plain text.
/// - `Err(LsToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, LsToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the ls tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, LsToolError> {
    // Parse the arguments from path attr + body.
    let args = LsArgs::parse(&args_json).map_err(LsToolError::ArgsParse)?;

    // Emit progress so the UI shows "Listing {path}" while waiting.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "ls",
            Some(args.path.clone()),
            format!("Listing {}", args.path),
        ),
    );

    // Execute the listing (always Ok — errors are inline in the text output).
    Ok(executor::execute(call_id, args).await)
}
