//! Executor for the todo_create tool — handles todo item creation and validation.
//!
//! This module contains the core logic for validating task descriptions,
//! creating new todo items in the store, and returning the result.

use crate::args::TodoCreateArgs;
use crate::output::TodoCreateOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::TodoStore;

/// Executes the todo_create tool with the given arguments.
///
/// Validates the task description, creates a new todo item in the store,
/// and returns the created item with its assigned ID and the updated total count.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized todo_create arguments containing content and optional priority.
/// - `store`: Mutable reference to the TodoStore where the item will be created.
///
/// # Returns
/// A `ToolResult` with either success (JSON TodoCreateOutput) or failure (Text error message).
pub async fn execute(
    call_id: ToolCallId,
    args: TodoCreateArgs,
    store: &mut TodoStore,
) -> ToolResult {
    // Step 1: Validate content is non-empty after trim.
    // An empty task description is a mistake by the model.
    let trimmed = args.content.trim();
    if trimmed.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_create".to_string(),
            content: ToolContent::Text("content is empty".to_string()),
            is_error: true,
        };
    }

    // Step 2: Create the todo item in the store.
    // The store assigns a unique ID and sets default status (Pending) and priority (Medium if not provided).
    let item = store.create(trimmed.to_string(), args.priority);

    // Step 3: Construct the output with the created item and total count.
    let output = TodoCreateOutput {
        item,
        total: store.len(),
    };

    // Step 4: Return success with JSON output.
    ToolResult {
        call_id,
        name: "todo_create".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!(output)),
        ),
        is_error: false,
    }
}
