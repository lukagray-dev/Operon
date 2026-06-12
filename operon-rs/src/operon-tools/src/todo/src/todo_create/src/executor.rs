//! Executor for the todo_create tool — handles todo item creation.
//!
//! This module contains the core logic for creating new todo items in the store
//! and returning the formatted plain-text result.

use crate::args::TodoCreateArgs;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::{TodoStatus, TodoStore};

/// Executes the todo_create tool with the given arguments.
///
/// Creates a new todo item in the store and returns the created item description
/// and overall status counts.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The parsed todo_create arguments.
/// - `store`: Mutable reference to the TodoStore where the item will be created.
///
/// # Returns
/// A `ToolResult` with plain-text content (ToolContent::Text).
pub async fn execute(
    call_id: ToolCallId,
    args: TodoCreateArgs,
    store: &mut TodoStore,
) -> ToolResult {
    // Step 1: Create the todo item in the store.
    let item = store.create(args.todo, args.priority);

    // Step 2: Retrieve all items to compute status counts.
    let all_items = store.list();
    let total = all_items.len();
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

    // Step 3: Format output as plain text.
    let text = format!(
        "Created #{}: {} [{}]\nTotal: {} ({} pending, {} in progress, {} completed)",
        item.id, item.content, item.priority, total, pending, in_progress, completed
    );

    // Step 4: Return success with plain-text output.
    ToolResult {
        call_id,
        name: "todo_create".to_string(),
        content: ToolContent::Text(text),
        is_error: false,
        read_paths: None,
    }
}
