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
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "Todo item id."
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
            }
        },
        "required": ["id"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_update".to_string(),
            description: "Updates an existing todo item by id. Pass `id` and any of: `content` (new text), \
                          `status` (\"pending\", \"in_progress\", \"completed\"), `priority` (\"high\", \"medium\", \"low\"). \
                          Only provided fields are updated. Mark items \"in_progress\" when starting, \"completed\" when done."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "todo_update".to_string(),
            description: "\
Updates an existing todo item by ID. Supports partial updates — only provided fields are changed. \
Use this to mark items in_progress as you start work, and completed when done.

## Input shapes

`id` (required, string): The ID of the item to update. Must match an existing item ID (as a string: \
\"1\", \"2\", \"3\", ...). If the ID is not found, the tool returns an error.

`content` (optional, string, non-empty): New content for the task. If provided, must be non-empty \
after trim. If not provided, the content is not changed. Use this to clarify or update the task \
description as work progresses.

`status` (optional, string, enum): New status for the task. Valid values: \"pending\", \"in_progress\", \
\"completed\". If not provided, the status is not changed. Typical workflow: pending → in_progress \
(when starting work) → completed (when done).

`priority` (optional, string, enum): New priority for the task. Valid values: \"high\", \"medium\", \
\"low\". If not provided, the priority is not changed. Use this to re-prioritize tasks as work \
progresses or new information emerges.

## Partial update semantics

Only provided fields are updated. Fields set to None (or omitted) are not changed. This allows \
flexible updates:
- Update only status: `{\"id\": \"1\", \"status\": \"in_progress\"}`
- Update only content: `{\"id\": \"1\", \"content\": \"New description\"}`
- Update multiple fields: `{\"id\": \"1\", \"status\": \"completed\", \"priority\": \"high\"}`

## Status transition workflow

Recommended workflow for task lifecycle:
1. Create item with status \"pending\" (default)
2. Update to \"in_progress\" when starting work
3. Update to \"completed\" when done

This workflow provides clear visibility into work progress.

## Error cases

- ID not found: \"todo not found: id 'X'\" — use `todo_list` to find valid IDs
- No fields to update: \"no fields to update — provide at least one of: content, status, priority\" — \
  provide at least one field to update
- Empty content: \"content is empty\" — provide non-empty content
- Malformed JSON: \"failed to deserialize tool arguments: ...\" — check the JSON shape
- Invalid status/priority values: \"failed to deserialize tool arguments: ...\" — use valid enum values

## Example calls

### Mark a task in progress
```json
{
  \"id\": \"1\",
  \"status\": \"in_progress\"
}
```
Result: Item with id \"1\" now has status \"in_progress\", other fields unchanged

### Mark a task completed
```json
{
  \"id\": \"1\",
  \"status\": \"completed\"
}
```
Result: Item with id \"1\" now has status \"completed\"

### Update content and priority
```json
{
  \"id\": \"2\",
  \"content\": \"Fix the critical login bug\",
  \"priority\": \"high\"
}
```
Result: Item with id \"2\" has new content and priority, status unchanged

### Clarify task description
```json
{
  \"id\": \"3\",
  \"content\": \"Implement grep tool with regex support and output formatting\"
}
```
Result: Item with id \"3\" has updated content, status and priority unchanged"
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the todo_update tool.
///
/// Returns a `ToolResult` with either success (JSON TodoUpdateOutput) or failure (Text error message).
/// Returns `Err(TodoUpdateToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
/// - `store`: Mutable reference to the TodoStore where the item will be updated.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(TodoUpdateToolError::ArgsParse)` if the arguments are malformed.
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
            Some(args.id.clone()),
            format!("Updating todo {}", args.id),
        ),
    );

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
