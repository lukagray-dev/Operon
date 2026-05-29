//! # operon-tools-fs-edit
//!
//! Implements the `edit` tool for the Operon agent's filesystem group.
//!
//! Edits an existing file by replacing exact text. Supports:
//! - Multi-hunk edits (one or more old_string→new_string replacements per call)
//! - Exact-string matching (zero or multiple matches are errors)
//! - Atomic writes (all hunks applied or none)
//! - In-order hunk application (later hunks see post-edit content from earlier hunks)
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

#[cfg(test)]
mod tests;

pub use args::{EditArgs, EditHunk};
pub use error::EditToolError;
pub use output::EditOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::TieredToolDefinition;
use serde_json::json;

/// Returns the tiered tool definition for the `edit` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraint (exact-string matching).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and common mistakes.
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
                            "description": "Exact text to replace. Must appear exactly once in the file."
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement text. Must differ from old_string."
                        }
                    },
                    "required": ["old_string", "new_string"]
                },
                "description": "One or more edits to apply in order. All applied atomically."
            }
        },
        "required": ["path", "edits"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "edit".to_string(),
            description: "Edits an existing file by replacing exact text. \
                          Pass `path` (absolute file path) and `edits` (array of {old_string, new_string} pairs). \
                          Each old_string must match exactly once — zero matches means the file changed (re-read and retry), \
                          multiple matches means old_string is ambiguous (add more surrounding context). \
                          All edits apply atomically."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "edit".to_string(),
            description: "\
Edits an existing file by replacing exact text. All edits are applied atomically — if any hunk fails, the file is NOT modified.

## Input shapes

`path` (required, string): Absolute path to the file to edit. Also accepted as \"file_path\" for compatibility.

`edits` (required, array, min 1 item): One or more edits to apply in order.
Each edit is an object with:
  - `old_string` (required, string): Exact text to find. Must match exactly once.
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

### Multi-hunk edit (three separate regions)
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

## Exact-string matching

Each old_string must match exactly once in the file. This ensures determinism and prevents silent partial edits.

- **Zero matches**: The file changed since you last read it. Re-read the file, then retry.
- **Multiple matches**: old_string is ambiguous. Include more surrounding context (function signature, preceding comment, more unique lines) to make it unique.

## Hunk application order

Edits are applied in array order on the in-memory string. Later hunks see post-edit content from earlier hunks.

Example: if hunk 0 replaces \"foo\" with \"bar\", and hunk 1 searches for \"bar\", hunk 1 will find the result of hunk 0.

If hunks touch overlapping regions, old_string for hunk N must match the state after hunks 0..N-1 have been applied.

## Atomic writes

All-or-nothing: if any hunk fails, the file is NOT modified at all. This prevents partial edits on disk.

## Whitespace exactness

Tabs vs spaces, trailing newlines, indentation — must match exactly as seen in read output.

**Important**: The line number prefix in read output (e.g., \"  123 | \") is display-only and must NOT be included in old_string.

## Common mistakes

### Mistake #1: old_string is too short and matches multiple places
Example: searching for just `}` or `return;` or a blank line.
Fix: Include the surrounding function name, comment, or more unique context.

### Mistake #2: Not re-reading after an external edit
If the file changed on disk (e.g., another tool or editor modified it), your old_string may no longer match.
Fix: Re-read the file with the read tool, then retry the edit.

### Mistake #3: Including the line number prefix from read output
Read output shows: \"  123 | fn foo() {\"
The \"  123 | \" prefix is display-only. old_string should be just: \"fn foo() {\"

## Error messages

- \"edits array must contain at least one hunk\" → Pass at least one edit.
- \"hunk N: old_string and new_string are identical\" → new_string must differ from old_string.
- \"hunk N: old_string not found in file\" → File changed since last read. Re-read and retry.
- \"hunk N: old_string matched K times — ambiguous\" → Add more context to make old_string unique.
- \"failed to read file: ...\" → File doesn't exist or permission denied.
- \"failed to write temp file: ...\" → Disk full or permission denied. File was not modified.
- \"failed to rename temp file to target: ...\" → Atomic rename failed. File was not modified."
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the edit tool.
///
/// Returns a `ToolResult` with either success (JSON EditOutput) or failure (Text error message).
/// Returns `Err(EditToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(EditToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_fs_edit::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({
///         "path": "/tmp/test.txt",
///         "edits": [{"old_string": "a", "new_string": "b"}]
///     })
/// ).await.unwrap();
/// assert_eq!(result.name, "edit");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, EditToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: EditArgs = serde_json::from_value(args_json)?;

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can unwrap safely.
    Ok(executor::execute(call_id, args).await)
}
