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
//! use operon_context_normalize::tools::ToolCallId;
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

#[cfg(test)]
mod tests;

pub use args::TodoUpdateArgs;
pub use error::TodoUpdateToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, TodoStore, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `todo_update` tool.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_update".to_string(),
            description: "Updates an existing todo item. Call format: <todo_update id=\"1\" todo=\"new content\" status=\"in_progress\" priority=\"high\"> \
                          `id` is required. Provide at least one of: `todo`, `status`, `priority`."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "todo_update".to_string(),
            description: "\
Updates an existing todo item by ID. Supports partial updates — only provided fields are changed.

## Call format

<todo_update id=\"1\" todo=\"new content\" status=\"in_progress\" priority=\"high\">

All attribute values are strings. The tool tag has no body.

## Attributes

`id` (required, string): The ID of the item to update. Must match an existing item ID.

`todo` (optional, string): New task description. If provided, must be non-empty after trim.

`status` (optional, string): New status for the task. Valid values: \"pending\", \"in_progress\", \"completed\".

`priority` (optional, string): New priority for the task. Valid values: \"high\", \"medium\", \"low\".

## Partial update semantics

Only provided fields are updated. At least one of `todo`, `status`, or `priority` must be specified.

## Output format

Plain text:
Updated #{id}: {todo} [{status}, {priority}]

## Error cases

- ID not found: \"todo not found: id 'X'\"
- No fields to update: \"no fields to update — provide at least one of: todo, status, priority\"
- Empty todo: \"todo is empty\"
- Malformed args: \"failed to parse tool arguments: ...\""
                .to_string(),
        },
    }
}

/// Parses `args_json` and executes the todo_update tool.
///
/// Returns a `ToolResult` with plain-text content (ToolContent::Text) on success.
/// Returns `Err(TodoUpdateToolError::ArgsParse)` if parsing attributes fails.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments.
/// - `store`: Mutable reference to the TodoStore.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
) -> Result<ToolResult, TodoUpdateToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Parses `args_json` and executes the todo_update tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, TodoUpdateToolError> {
    // Parse the arguments manually. If this fails, return an ArgsParse error.
    let args = TodoUpdateArgs::parse(&args_json).map_err(TodoUpdateToolError::ArgsParse)?;

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
