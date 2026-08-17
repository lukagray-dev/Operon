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
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, TodoStore, ToolProgress, ToolProgressEmitter,
};
use serde_json::json;

/// Returns the tiered tool definition for the `todo_create` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and key parameters.
/// - `detailed`: sent after a malformed call. Clean explanation with input shapes
///   and response format.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "content": {
                "type": "string",
                "minLength": 1,
                "description": "Task description for single creation. Imperative form: 'Implement the grep tool'."
            },
            "priority": {
                "type": "string",
                "enum": ["high", "medium", "low"],
                "description": "Priority level for single creation. Default: medium."
            },
            "todos": {
                "type": "array",
                "description": "Array of todo items to create at once.",
                "items": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "minLength": 1,
                            "description": "Task description (e.g. 'Fix login bug')."
                        },
                        "priority": {
                            "type": "string",
                            "enum": ["high", "medium", "low"],
                            "description": "Priority level (default: 'medium')."
                        }
                    },
                    "required": ["content"]
                }
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_create".to_string(),
            description: "Creates one or multiple todo items. \
                          Pass `content` (or `todos` array) with optional `priority` (\"high\", \"medium\", \"low\"). \
                          Returns created items with assigned IDs."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "todo_create".to_string(),
            description: "\
Creates one or multiple todo items in a single call. Returns created item(s) with assigned IDs.

## Input shapes

1. Single task:
   `{\"content\": \"Fix login bug\", \"priority\": \"high\"}`
   `{\"content\": \"Write unit tests\"}`

2. Multiple tasks in batch:
   `{\"todos\": [{\"content\": \"Fix login bug\", \"priority\": \"high\"}, {\"content\": \"Write tests\", \"priority\": \"medium\"}]}`

3. Array of task strings:
   `{\"todos\": [\"Implement parser\", \"Add error handling\"]}`

## Response format

Returns JSON object with `items` list and `total` count:
`{\"items\": [{\"id\": \"1\", \"content\": \"...\", \"status\": \"pending\", \"priority\": \"high\"}], \"total\": 1}`"
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
/// assert!(!result.unwrap().is_error);
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
) -> Result<ToolResult, TodoCreateToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the todo_create tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, TodoCreateToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: TodoCreateArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "todo_create", None, "Creating todo item"),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error), so we can wrap it in Ok.
    Ok(executor::execute(call_id, args, store).await)
}
