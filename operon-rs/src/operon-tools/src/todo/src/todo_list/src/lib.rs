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
pub use output::TodoListOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, TodoStore, ToolProgress, ToolProgressEmitter};
use serde_json::json;

/// Returns the canonical tool definition for the `todo_list` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Clear parameter descriptions and enums for status and priority filters.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the parameters schema for listing todos here.
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

    ToolDefinition {
        name: "todo_list".to_string(),
        description: "Returns the current todo list with status counts. \
                      Optionally filter by `status` (\"pending\", \"in_progress\", \"completed\") \
                      or `priority` (\"high\", \"medium\", \"low\")."
            .to_string(),
        parameters,
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
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the todo_list tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &TodoStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, TodoListToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: TodoListArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "todo_list", None, "Listing todos"),
    );

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
