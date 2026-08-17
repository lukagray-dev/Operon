//! # operon-tools-todo-delete
//!
//! Implements the `todo_delete` tool for the Operon agent's todo group.
//!
//! Deletes a todo item by ID. Prefer marking items "completed" over deleting them —
//! deletion is for items added by mistake.
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_todo_delete::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use operon_tools_core::TodoStore;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let mut store = TodoStore::new();
//! let args = json!({
//!     "id": "1"
//! });
//! let result = execute(
//!     ToolCallId("call_123".to_string()),
//!     args,
//!     &mut store
//! ).await;
//! # }
//! ```

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::TodoDeleteArgs;
pub use error::TodoDeleteToolError;
pub use output::TodoDeleteOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, TodoStore, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `todo_delete` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and key parameters.
/// - `detailed`: sent after a malformed call. Clean explanation with input shapes
///   and response format.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "ID of the single todo item to delete."
            },
            "ids": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Array of item IDs to delete in batch."
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_delete".to_string(),
            description: "Deletes one or multiple todo items by ID. Pass `id` (or `ids` array). \
                          Returns deleted IDs and remaining count. Note: prefer marking items \
                          \"completed\" over deleting them — deletion is for items added by mistake."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "todo_delete".to_string(),
            description: "\
Deletes one or multiple todo items by ID. Returns deleted ID(s) and remaining count.

## Input shapes

1. Single item deletion:
   `{\"id\": \"1\"}`

2. Batch deletion of multiple IDs:
   `{\"ids\": [\"1\", \"2\", \"3\"]}`

## Response format

Returns JSON object with deleted `ids` and `remaining` count:
`{\"ids\": [\"1\", \"2\"], \"remaining\": 3}`"
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the todo_delete tool.
///
/// Returns a `ToolResult` with either success (JSON TodoDeleteOutput) or failure (Text error message).
/// Returns `Err(TodoDeleteToolError::ArgsParse)` only if the top-level JSON shape is invalid.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
) -> Result<ToolResult, TodoDeleteToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the todo_delete tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, TodoDeleteToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: TodoDeleteArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "todo_delete",
            None,
            "Deleting todo item(s)".to_string(),
        ),
    );

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
