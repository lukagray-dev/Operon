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
use operon_tools_core::{TieredToolDefinition, TodoStore};
use serde_json::json;

/// Returns the tiered tool definition for the `todo_delete` tool.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "Id of the todo item to delete."
            }
        },
        "required": ["id"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_delete".to_string(),
            description: "Deletes a todo item by id. Pass `id` (from todo_create or todo_list output). \
                          Returns the id that was deleted and the remaining count. Prefer marking items \
                          \"completed\" over deleting them — deletion is for items added by mistake."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "todo_delete".to_string(),
            description: "\
Deletes a todo item by ID. Returns the deleted ID and the remaining count. Prefer marking items \
\"completed\" over deleting them — deletion is for items added by mistake.

## Input shapes

`id` (required, string): The ID of the item to delete. Must match an existing item ID (as a string: \
\"1\", \"2\", \"3\", ...). If the ID is not found, the tool returns an error.

## Output

Returns a JSON object with:
- `id`: The ID that was deleted
- `remaining`: Total number of todos remaining after deletion

## When to delete vs mark completed

Prefer marking items \"completed\" over deleting them. Deletion is for items added by mistake.

- **Mark completed**: Use `todo_update` with `status: \"completed\"` for tasks that were done. \
  This preserves task history and visibility into what was accomplished.
- **Delete**: Use `todo_delete` only for items added by mistake or that are no longer relevant. \
  Deletion removes the item entirely.

## Error cases

- ID not found: \"todo not found: id 'X'\" — use `todo_list` to find valid IDs
- Malformed JSON: \"failed to deserialize tool arguments: ...\" — check the JSON shape

## Example calls

### Delete an item added by mistake
```json
{
  \"id\": \"3\"
}
```
Result: Item with id \"3\" is deleted, remaining count is updated

### Delete multiple items
First call:
```json
{
  \"id\": \"1\"
}
```
Then:
```json
{
  \"id\": \"2\"
}
```
Result: Both items deleted, remaining count decremented each time"
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the todo_delete tool.
///
/// Returns a `ToolResult` with either success (JSON TodoDeleteOutput) or failure (Text error message).
/// Returns `Err(TodoDeleteToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
/// - `store`: Mutable reference to the TodoStore where the item will be deleted.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(TodoDeleteToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
) -> Result<ToolResult, TodoDeleteToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: TodoDeleteArgs = serde_json::from_value(args_json)?;

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
