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
            description: "Deletes a todo item by id. Call format: <todo_delete id=\"1\">"
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "todo_delete".to_string(),
            description: "\
Deletes a todo item by ID. Returns a confirmation of deletion and the remaining count.

## Call format

<todo_delete id=\"1\">

All attribute values are strings. The tool tag has no body.

## Attributes

`id` (required, string): The ID of the item to delete. Must match an existing item ID.

## Output format

Plain text:
Deleted #{id}. {remaining} todo(s) remaining.

## Error cases

- ID not found: \"todo not found: id 'X'\"
- Malformed args: \"failed to parse tool arguments: ...\""
                .to_string(),
            parameters,
        },
    }
}

/// Parses `args_json` and executes the todo_delete tool.
///
/// Returns a `ToolResult` with plain-text content (ToolContent::Text) on success.
/// Returns `Err(TodoDeleteToolError::ArgsParse)` if parsing attributes fails.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments.
/// - `store`: Mutable reference to the TodoStore.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
) -> Result<ToolResult, TodoDeleteToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Parses `args_json` and executes the todo_delete tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, TodoDeleteToolError> {
    // Parse the arguments manually. If this fails, return an ArgsParse error.
    let args = TodoDeleteArgs::parse(&args_json).map_err(TodoDeleteToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "todo_delete",
            Some(args.id.clone()),
            format!("Deleting todo {}", args.id),
        ),
    );

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
