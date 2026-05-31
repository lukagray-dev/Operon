//! # operon-tools-todo-list
//!
//! Implements the `todo_list` tool for the Operon agent's todo group.
//!
//! Returns the current todo list with optional filtering by status or priority.
//! Always includes status counts for quick overview of work progress.
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_todo_list::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use operon_tools_core::TodoStore;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let store = TodoStore::new();
//! let args = json!({
//!     "status": "pending"
//! });
//! let result = execute(
//!     ToolCallId("call_123".to_string()),
//!     args,
//!     &store
//! ).await;
//! # }
//! ```

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::TodoListArgs;
pub use error::TodoListToolError;
pub use executor::execute;
pub use output::TodoListOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::TieredToolDefinition;
use serde_json::json;

/// Returns the tiered tool definition for the `todo_list` tool.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["pending", "in_progress", "completed"],
                "description": "Optional filter by status."
            },
            "priority": {
                "type": "string",
                "enum": ["high", "medium", "low"],
                "description": "Optional filter by priority."
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_list".to_string(),
            description: "Returns the current todo list. Optionally filter by `status` (\"pending\", \
                          \"in_progress\", \"completed\") or `priority` (\"high\", \"medium\", \"low\"). \
                          Always call this at the start of a session to check your current task plan."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "todo_list".to_string(),
            description: "\
Returns the current todo list with optional filtering by status or priority. Always includes \
status counts for quick overview of work progress.

## Input shapes

`status` (optional, string, enum): Filter by status. Valid values: \"pending\", \"in_progress\", \
\"completed\". If not provided, returns all todos regardless of status.

`priority` (optional, string, enum): Filter by priority. Valid values: \"high\", \"medium\", \"low\". \
If not provided, returns all todos regardless of priority.

Both filters can be combined — if both are provided, only items matching both filters are returned.

## Output

Returns a JSON object with:
- `items`: Array of TodoItem objects matching the filters (or all items if no filters)
- `total`: Total number of todos in the store (unfiltered count)
- `pending`: Count of items with status \"pending\" (unfiltered)
- `in_progress`: Count of items with status \"in_progress\" (unfiltered)
- `completed`: Count of items with status \"completed\" (unfiltered)

The status counts are always computed from the full unfiltered list, giving you a complete \
overview of work progress even when filtering.

## Empty list

An empty list is valid, not an error. This occurs when:
- No todos have been created yet
- All todos have been deleted
- Filters are applied but no items match

## Session scope

Todos are session-scoped — they exist for the duration of the agent session only. When the session \
ends, todos are lost. Compaction does NOT clear todos — the task plan survives summarization.

## Workflow guidance

- Call this at the start of a session to check your current task plan
- Use status filters to focus on specific work: \"pending\" for unstarted, \"in_progress\" for active
- Use priority filters to focus on urgent work: \"high\" for critical tasks
- Check status counts to understand overall progress
- Mark items \"in_progress\" as you start work, \"completed\" when done
- Use `todo_create` to add new tasks
- Use `todo_update` to change status or priority
- Use `todo_delete` only for items added by mistake

## Error cases

- Malformed JSON: \"failed to deserialize tool arguments: ...\" — check the JSON shape
- Invalid status/priority values: \"failed to deserialize tool arguments: ...\" — use valid enum values

## Example calls

### List all todos
```json
{}
```
Result: All items with status counts

### List pending todos
```json
{
  \"status\": \"pending\"
}
```
Result: Only items with status \"pending\"

### List high-priority todos
```json
{
  \"priority\": \"high\"
}
```
Result: Only items with priority \"high\"

### List high-priority pending todos
```json
{
  \"status\": \"pending\",
  \"priority\": \"high\"
}
```
Result: Only items matching both filters"
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the todo_list tool.
///
/// Returns a `ToolResult` with success (JSON TodoListOutput). Never returns is_error: true —
/// an empty list is valid, not an error.
/// Returns `Err(TodoListToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
/// - `store`: Reference to the TodoStore (immutable — list doesn't mutate).
///
/// # Returns
/// - `Ok(ToolResult)` with success (both as Ok, not Err).
/// - `Err(TodoListToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &TodoStore,
) -> Result<ToolResult, TodoListToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: TodoListArgs = serde_json::from_value(args_json)?;

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
