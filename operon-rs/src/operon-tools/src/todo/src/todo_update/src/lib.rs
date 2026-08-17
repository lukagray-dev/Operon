//! # operon-tools-todo-update
//!
//! Implements the `todo_update` tool for the Operon agent's todo group.
//!
//! Updates an existing todo item by ID. Supports partial updates — only provided
//! fields are changed. Use this to mark items in_progress as you start work,
//! and completed when done.
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_todo_update::{definition, execute};
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
//!     "id": "1",
//!     "status": "in_progress"
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

pub use args::TodoUpdateArgs;
pub use error::TodoUpdateToolError;
pub use output::TodoUpdateOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, TodoStore, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `todo_update` tool.
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
                "description": "Todo item id (single update)."
            },
            "ids": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Array of item IDs to update with the same status/priority in bulk."
            },
            "content": {
                "type": "string",
                "minLength": 1,
                "description": "New content. None = no change."
            },
            "status": {
                "type": "string",
                "enum": ["pending", "in_progress", "completed"],
                "description": "New status. None = no change."
            },
            "priority": {
                "type": "string",
                "enum": ["high", "medium", "low"],
                "description": "New priority. None = no change."
            },
            "todos": {
                "type": "array",
                "description": "Array of distinct todo item updates in batch.",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Item ID to update." },
                        "content": { "type": "string", "minLength": 1, "description": "New content." },
                        "status": { "type": "string", "enum": ["pending", "in_progress", "completed"], "description": "New status." },
                        "priority": { "type": "string", "enum": ["high", "medium", "low"], "description": "New priority." }
                    },
                    "required": ["id"]
                }
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_update".to_string(),
            description: "Updates one or multiple todo items. Pass `id` (or `ids` / `todos` array) and any of: \
                          `status` (\"pending\", \"in_progress\", \"completed\"), `priority` (\"high\", \"medium\", \"low\"), `content`. \
                          Returns updated items."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "todo_update".to_string(),
            description: "\
Updates one or multiple todo items in a single call. Supports partial updates.

## Input shapes

1. Single task update:
   `{\"id\": \"1\", \"status\": \"in_progress\"}`
   `{\"id\": \"2\", \"status\": \"completed\", \"priority\": \"high\"}`

2. Batch update of distinct tasks:
   `{\"todos\": [{\"id\": \"1\", \"status\": \"completed\"}, {\"id\": \"2\", \"status\": \"in_progress\"}]}`

3. Bulk status update across multiple IDs:
   `{\"ids\": [\"1\", \"2\", \"3\"], \"status\": \"completed\"}`

## Response format

Returns JSON object with updated `items` list:
`{\"items\": [{\"id\": \"1\", \"content\": \"...\", \"status\": \"completed\", \"priority\": \"high\"}]}`"
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the todo_update tool.
///
/// Returns a `ToolResult` with either success (JSON TodoUpdateOutput) or failure (Text error message).
/// Returns `Err(TodoUpdateToolError::ArgsParse)` only if the top-level JSON shape is invalid.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
) -> Result<ToolResult, TodoUpdateToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the todo_update tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, TodoUpdateToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: TodoUpdateArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "todo_update",
            None,
            "Updating todo item(s)".to_string(),
        ),
    );

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
