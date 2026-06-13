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
//! use operon_context_normalize::tools::ToolCallId;
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

#[cfg(test)]
mod tests;

pub use args::TodoListArgs;
pub use error::TodoListToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, TodoStore, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `todo_list` tool.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_list".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses `args_json` and executes the todo_list tool.
///
/// Returns a `ToolResult` with plain-text content (ToolContent::Text).
/// Returns `Err(TodoListToolError::ArgsParse)` if parsing attributes fails.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments.
/// - `store`: Reference to the TodoStore.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &TodoStore,
) -> Result<ToolResult, TodoListToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Parses `args_json` and executes the todo_list tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &TodoStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, TodoListToolError> {
    // Parse the arguments manually. If this fails, return an ArgsParse error.
    let args = TodoListArgs::parse(&args_json).map_err(TodoListToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "todo_list", None, "Listing todos"),
    );

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
