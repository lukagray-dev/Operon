//! Executor for the todo_list tool — handles listing and filtering todos.

use crate::args::TodoListArgs;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::{TodoStatus, TodoStore};

/// Executes the todo_list tool with the given arguments.
///
/// Retrieves all todos from the store, applies optional status and priority filters,
/// and returns the filtered list as plain text along with status counts.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The parsed todo_list arguments containing optional filters.
/// - `store`: Reference to the TodoStore.
///
/// # Returns
/// A `ToolResult` with plain-text content (ToolContent::Text).
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

    // Step 4: Construct the plain-text output.
    if all_items.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_list".to_string(),
            content: ToolContent::Text("No todos yet.".to_string()),
            is_error: false,
            read_paths: None,
        };
    }

    let summary = format!(
        "Total: {} ({} pending, {} in progress, {} completed)",
        all_items.len(),
        pending,
        in_progress,
        completed
    );

    let text = if filtered_items.is_empty() {
        format!("No todos match the given filters.\n\n{}", summary)
    } else {
        let items_text = filtered_items
            .into_iter()
            .map(|item| {
                format!(
                    "#{} [{}] [{}] {}",
                    item.id, item.status, item.priority, item.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n\n{}", items_text, summary)
    };

    ToolResult {
        call_id,
        name: "todo_list".to_string(),
        content: ToolContent::Text(text),
        is_error: false,
        read_paths: None,
    }
}
