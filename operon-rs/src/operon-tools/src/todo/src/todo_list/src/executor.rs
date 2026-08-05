//! Executor for the todo_list tool — handles listing and filtering todos.

use crate::args::TodoListArgs;
use crate::output::TodoListOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::{TodoStatus, TodoStore};

/// Executes the todo_list tool with the given arguments.
///
/// Retrieves all todos from the store, applies optional status and priority filters,
/// and returns the filtered list along with status counts (always from the full unfiltered list).
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized todo_list arguments containing optional filters.
/// - `store`: Reference to the TodoStore (immutable — list doesn't mutate).
///
/// # Returns
/// A `ToolResult` with success (JSON TodoListOutput). Never returns is_error: true —
/// an empty list is valid, not an error.
pub async fn execute(call_id: ToolCallId, args: TodoListArgs, store: &TodoStore) -> ToolResult {
    // Step 1: Get all items from the store.
    let all_items = store.list();

    // Step 2: Apply filters if provided.
    let filtered_items: Vec<_> = all_items
        .iter()
        .filter(|item| {
            // Apply status filter if provided.
            if let Some(ref status) = args.status {
                if item.status != *status {
                    return false;
                }
            }
            // Apply priority filter if provided.
            if let Some(ref priority) = args.priority {
                if item.priority != *priority {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    // Step 3: Compute status counts from the full unfiltered list.
    let pending = all_items
        .iter()
        .filter(|i| i.status == TodoStatus::Pending)
        .count();
    let in_progress = all_items
        .iter()
        .filter(|i| i.status == TodoStatus::InProgress)
        .count();
    let completed = all_items
        .iter()
        .filter(|i| i.status == TodoStatus::Completed)
        .count();

    // Step 4: Construct the output.
    let output = TodoListOutput {
        items: filtered_items,
        total: all_items.len(),
        pending,
        in_progress,
        completed,
    };

    // Step 5: Return success with JSON output. Never is_error: true — empty list is valid.
    ToolResult {
        call_id,
        name: "todo_list".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!(output)),
        ),
        is_error: false,
    }
}
