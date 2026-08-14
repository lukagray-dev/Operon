//! # operon-tools-fs-edit
//!
//! Hey friend! Implements the `edit` tool for the Operon agent's filesystem group.
//!
//! Edits an existing file by applying an array of `old_string` -> `new_string` replacement hunks.
//! Supports:
//! - Multi-hunk edits (one or more hunks per call, applied sequentially in-memory)
//! - 6-pass fuzzy sequence seeking (exact -> rstrip -> trim -> Unicode normalization -> case insensitivity -> case + Unicode)
//! - Partial-success execution (successful hunks are written to disk; failed hunks reported in structured diagnostics)
//! - Atomic writes (committed in a single atomic temp-file rename when at least one hunk succeeds)
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
//!     "edits": [
//!         {
//!             "old_string": "fn old_name() {",
//!             "new_string": "fn new_name() {"
//!         }
//!     ]
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
mod seek_sequence;

#[cfg(test)]
mod tests;

pub use args::{EditArgs, EditHunk};
pub use error::EditToolError;
pub use output::{EditOutput, HunkFailure};

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `edit` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the `old_string`/`new_string` array schema.
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   worked examples, fuzzy matching behavior, and partial-success semantics.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to the file to edit. Also accepted as file_path."
            },
            "edits": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "properties": {
                        "old_string": {
                            "type": "string",
                            "description": "Exact or fuzzy-matchable text to replace. Must appear uniquely in the file."
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement text. Must differ from old_string."
                        }
                    },
                    "required": ["old_string", "new_string"]
                },
                "description": "One or more edits to apply in order. All successful edits are committed atomically."
            }
        },
        "required": ["path", "edits"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "edit".to_string(),
            description: "Edits an existing file by replacing text hunks. \
                          Pass `path` (absolute file path) and `edits` (array of {old_string, new_string} pairs). \
                          Each old_string is located using exact & fuzzy sequence matching (exact -> space trim -> Unicode punctuation -> case insensitivity). \
                          If some hunks match and others fail, successful hunks are written to disk and failed hunks are reported back for retry."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "edit".to_string(),
            description: "\
Edits an existing file by replacing text hunks.

## Input shapes

`path` (required, string): Absolute path to the file to edit. Also accepted as \"file_path\" for compatibility.

`edits` (required, array, min 1 item): One or more edits to apply in order.
Each edit is an object with:
  - `old_string` (required, string): Exact or unique text to find. Must match uniquely.
  - `new_string` (required, string): Replacement text. Must differ from old_string.

## Worked examples

### Single edit
```json
{
  \"path\": \"/path/to/file.rs\",
  \"edits\": [
    {
      \"old_string\": \"fn old_name() {\",
      \"new_string\": \"fn new_name() {\"
    }
  ]
}
```

### Multi-hunk edit
```json
{
  \"path\": \"/path/to/file.rs\",
  \"edits\": [
    {
      \"old_string\": \"import { oldFunc } from './lib';\",
      \"new_string\": \"import { newFunc } from './lib';\"
    },
    {
      \"old_string\": \"oldFunc(x, y)\",
      \"new_string\": \"newFunc(x, y)\"
    },
    {
      \"old_string\": \"// TODO: refactor oldFunc\",
      \"new_string\": \"// TODO: refactor newFunc\"
    }
  ]
}
```

## Matching Lenience & Fuzzy Fallback

The tool first attempts exact substring matching. If exact matching fails (0 matches), it uses a 6-pass fuzzy sequence seeker:
1. Exact byte-for-byte line match
2. Trailing whitespace ignored (rstrip)
3. Leading & trailing whitespace ignored (trim)
4. Unicode punctuation normalized (dashes, quotes, non-breaking spaces converted to ASCII)
5. Case-insensitive matching fallback
6. Case-insensitive Unicode normalisation

## Hunk Ordering & Partial Success Semantics

Edits are evaluated sequentially in array order against the running in-memory working buffer:
- Later hunks see changes made by earlier successful hunks.
- If some hunks match and others fail, all successfully matched hunks are written to disk!
- Failed hunks are reported back in `failures` with their index, `old_string`, and error reason so you only need to retry the specific hunks that failed.
- If zero hunks match, the file is NOT modified.

## Common Mistakes

1. **Ambiguous old_string**: searching for just `}` or `return;` matches multiple places. Include more surrounding lines to make it unique.
2. **Display prefix in old_string**: the line number prefix in read output (e.g., \"  123 | \") is display-only and must NOT be included in old_string."
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the edit tool.
///
/// Returns a `ToolResult` containing structured `EditOutput` JSON.
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
            format!("Editing {} ({} edit(s))", args.path, args.edits.len()),
        ),
    );

    Ok(executor::execute(call_id, args).await)
}
