//! # operon-tools-fs-edit
//!
//! Hey friend! Implements the `edit` tool for the Operon agent's filesystem group.
//!
//! Edits an existing file by applying unified-diff style patch hunks. Supports:
//! - Multi-hunk edits (one or more `@@` hunks per patch string)
//! - Fuzzy sequence seeking (exact → rstrip → trim → Unicode punctuation normalization → case insensitivity)
//! - Atomic writes (all hunks apply or none)
//! - In-order hunk application
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_fs_edit::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "path": "/path/to/file.rs",
//!     "patch": "@@ fn old_name()\n-fn old_name() {\n+fn new_name() {"
//! });
//! let result = execute(
//!     ToolCallId("call_123".to_string()),
//!     args
//! ).await.unwrap();
//! # }
//! ```

mod args;
mod chunk_parser;
mod error;
mod executor;
mod output;
mod seek_sequence;

#[cfg(test)]
mod tests;

pub use args::EditArgs;
pub use chunk_parser::{parse_patch_chunks, ChunkParseError, UpdateFileChunk};
pub use error::EditToolError;
pub use output::EditOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `edit` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the unified diff schema.
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and line prefix syntax.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to the file to edit. Also accepted as file_path."
            },
            "patch": {
                "type": "string",
                "description": "Unified-diff style patch body containing one or more @@ hunks."
            }
        },
        "required": ["path", "patch"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "edit".to_string(),
            description: "Edits an existing file by applying a unified-diff style patch body. \
                          Pass `path` (absolute file path) and `patch` (string containing @@ hunks with ' ' context, '-' removed, and '+' added lines). \
                          Hunks are located using fuzzy sequence matching (exact -> space trim -> Unicode punctuation -> case insensitivity). \
                          All edits apply atomically."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "edit".to_string(),
            description: "\
Edits an existing file by applying unified-diff style patch hunks. All edits are applied atomically — if any hunk fails, the file is NOT modified.

## Input shapes

`path` (required, string): Absolute path to the file to edit. Also accepted as \"file_path\" for compatibility.

`patch` (required, string): Unified-diff style patch text containing one or more hunks starting with `@@`.

Each hunk starts with `@@` or `@@ <context_text>` followed by lines prefixed with:
  - `' '` (space): Context line present in both original and modified file.
  - `'-'`: Line present in original file to be removed.
  - `'+'`: Line to be inserted into modified file.

## Worked examples

### Single edit hunk
```json
{
  \"path\": \"/path/to/file.rs\",
  \"patch\": \"@@ fn old_name()\\n-fn old_name() {\\n+fn new_name() {\"
}
```

### Multi-hunk edit
```json
{
  \"path\": \"/path/to/file.rs\",
  \"patch\": \"@@ import header\\n-import { oldFunc } from './lib';\\n+import { newFunc } from './lib';\\n@@ fn process()\\n-oldFunc(x, y)\\n+newFunc(x, y)\"
}
```

## Matching Lenience

Hunk matching uses fuzzy line sequence seeking (`seek_sequence`):
1. Exact byte-for-byte line match
2. Trailing whitespace ignored (rstrip)
3. Leading & trailing whitespace ignored (trim)
4. Unicode punctuation normalized (dashes, quotes, non-breaking spaces converted to ASCII)
5. Case-insensitive matching fallback

## Atomic Writes

All-or-nothing: if any hunk fails, the target file is NOT modified at all.

## Error Messages

- \"failed to parse patch: ...\" → Syntax error in patch format (missing @@ marker or unexpected line prefix).
- \"hunk N: old_string not found in file: ...\" → Context or target lines were not found. Re-read the file with the read tool and retry.
- \"failed to read file: ...\" → File does not exist or permission denied.
- \"failed to write temp file: ...\" → Permission denied or disk full."
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the edit tool.
///
/// Returns a `ToolResult` with either success (JSON EditOutput) or failure (Text error message).
/// Returns `Err(EditToolError::ArgsParse)` only if top-level JSON deserialization fails.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, EditToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Deserializes `args_json` and executes the edit tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, EditToolError> {
    let args: EditArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "edit",
            Some(args.path.clone()),
            format!("Editing {}", args.path),
        ),
    );

    Ok(executor::execute(call_id, args).await)
}
