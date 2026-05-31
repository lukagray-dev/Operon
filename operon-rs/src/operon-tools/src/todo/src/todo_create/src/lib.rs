//! # operon-tools-todo-create
//!
//! Implements the `todo_create` tool for the Operon agent's todo group.
//!
//! Creates a new todo item in the agent's session-scoped task list.
//! Each item is assigned a unique auto-incrementing ID and starts with
//! status "pending" and priority "medium" (or as specified).
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_todo_create::{definition, execute};
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
//!     "content": "Fix the login bug",
//!     "priority": "high"
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

pub use args::TodoCreateArgs;
pub use error::TodoCreateToolError;
pub use output::TodoCreateOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{TieredToolDefinition, TodoStore};
use serde_json::json;

/// Returns the tiered tool definition for the `todo_create` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints.
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "content": {
                "type": "string",
                "minLength": 1,
                "description": "Task description. Use imperative form: 'Implement the grep tool'."
            },
            "priority": {
                "type": "string",
                "enum": ["high", "medium", "low"],
                "description": "Priority level. Default: medium."
            }
        },
        "required": ["content"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_create".to_string(),
            description: "Creates a new todo item. Pass `content` (task description, imperative form) and \
                          optionally `priority` (\"high\", \"medium\", \"low\" — default: \"medium\"). Returns the \
                          created item with its assigned id. Use todo items to plan and track your work — \
                          create a todo list at the start of any multi-step task."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "todo_create".to_string(),
            description: "\
Creates a new todo item in the agent's session-scoped task list. Each item is assigned a unique \
auto-incrementing ID (as a string: \"1\", \"2\", \"3\", ...) and starts with status \"pending\" \
and priority \"medium\" (or as specified).

## Input shapes

`content` (required, string, non-empty): Task description. Use imperative form to describe what needs \
to be done. Examples: \"Fix the login bug\", \"Implement the grep tool\", \"Write unit tests for the \
parser\". The description should be concise but descriptive enough to understand the task at a glance. \
Whitespace is trimmed — leading and trailing spaces are removed. If the content is empty or \
whitespace-only, the tool returns an error.

`priority` (optional, string, enum): Priority level for the task. Valid values: \"high\", \"medium\", \
\"low\". Defaults to \"medium\" if not provided. Use \"high\" for urgent tasks that should be done first, \
\"medium\" for normal tasks, and \"low\" for tasks that can be deferred. Priority is informational — \
it helps organize and prioritize work but does not affect tool behavior.

## Output

Returns a JSON object with:
- `item`: The created TodoItem with fields: id (string), content (string), status (\"pending\"), \
  priority (as specified or \"medium\")
- `total`: Total number of todos in the store after creation

## Status and lifecycle

New items always start with status \"pending\". As work progresses, update the status to \
\"in_progress\" when starting work, and \"completed\" when done. Use the `todo_update` tool to \
change status.

## Session scope

Todos are session-scoped — they exist for the duration of the agent session only. When the session \
ends, todos are lost. Compaction does NOT clear todos — the task plan survives summarization.

## Workflow guidance

- Create a todo list at the start of any multi-step task to plan your work
- Use imperative form for task descriptions (\"Fix bug\" not \"Bug fix\")
- Set priority to \"high\" for urgent tasks, \"medium\" for normal, \"low\" for deferred
- Mark items \"in_progress\" when starting work, \"completed\" when done
- Use `todo_list` to check your current task plan
- Use `todo_update` to change status or priority as work progresses
- Use `todo_delete` only for items added by mistake — prefer marking \"completed\" for task history

## Error cases

- Empty content: \"content is empty\" — provide a non-empty task description
- Malformed JSON: \"failed to deserialize tool arguments: ...\" — check the JSON shape

## Example calls

### Create a high-priority task
```json
{
  \"content\": \"Fix the critical login bug\",
  \"priority\": \"high\"
}
```
Result: Item with id \"1\", status \"pending\", priority \"high\"

### Create a normal-priority task
```json
{
  \"content\": \"Write unit tests for the parser\"
}
```
Result: Item with id \"2\", status \"pending\", priority \"medium\" (default)

### Create multiple tasks
```json
{
  \"content\": \"Implement the grep tool\",
  \"priority\": \"high\"
}
```
Then:
```json
{
  \"content\": \"Add documentation\",
  \"priority\": \"low\"
}
```
Result: Two items with ids \"1\" and \"2\", different priorities"
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the todo_create tool.
///
/// Returns a `ToolResult` with either success (JSON TodoCreateOutput) or failure (Text error message).
/// Returns `Err(TodoCreateToolError::ArgsParse)` only if the top-level JSON shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
/// - `store`: Mutable reference to the TodoStore where the item will be created.
///
/// # Returns
/// - `Ok(ToolResult)` with either success or failure (both as Ok, not Err).
/// - `Err(TodoCreateToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_todo_create::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use operon_tools_core::TodoStore;
/// # use serde_json::json;
/// # async fn example() {
/// let mut store = TodoStore::new();
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({
///         "content": "Fix the login bug",
///         "priority": "high"
///     }),
///     &mut store
/// ).await;
/// assert!(!result.is_error);
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
) -> Result<ToolResult, TodoCreateToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: TodoCreateArgs = serde_json::from_value(args_json)?;

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can wrap it in Ok.
    Ok(executor::execute(call_id, args, store).await)
}
