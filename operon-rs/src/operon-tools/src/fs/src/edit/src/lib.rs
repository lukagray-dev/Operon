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
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use serde_json::json;

/// Returns the canonical tool definition for the `edit` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`path`, `edits`, `old_string`, `new_string`).
/// - Concise explanation of targeted hunk replacements and fuzzy matching.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the schema for file editing here.
    // The model passes the target path and a list of edits containing old_string and new_string.
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "File path to edit. Also accepted as file_path."
            },
            "edits": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "properties": {
                        "old_string": {
                            "type": "string",
                            "description": "Exact or uniquely matchable text to replace within the file."
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement text to insert in place of old_string."
                        }
                    },
                    "required": ["old_string", "new_string"]
                },
                "description": "One or more edits to apply in order."
            }
        },
        "required": ["path", "edits"]
    });

    ToolDefinition {
        name: "edit".to_string(),
        description: "Edits an existing file by replacing text hunks. \
                      Pass `path` (file path) and `edits` (array of {old_string, new_string} pairs). \
                      Each old_string is located using exact & fuzzy sequence matching (exact -> space trim -> Unicode punctuation -> case insensitivity). \
                      If some hunks match and others fail, successful hunks are written to disk and failed hunks are reported back for retry."
            .to_string(),
        parameters,
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
