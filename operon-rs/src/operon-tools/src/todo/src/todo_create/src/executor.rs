//! Executor for the todo_create tool — handles single and batch todo item creation.

use crate::args::TodoCreateArgs;
use crate::output::TodoCreateOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::TodoStore;

/// Executes the todo_create tool with the given arguments.
///
/// Validates all task descriptions, creates new todo item(s) in the store,
/// and returns the created item(s) with assigned IDs and total store count.
pub async fn execute(
    call_id: ToolCallId,
    args: TodoCreateArgs,
    store: &mut TodoStore,
) -> ToolResult {
    // Extract all items to create
    let items_to_create = args.into_items();

    if items_to_create.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_create".to_string(),
            content: ToolContent::Text("content is empty — provide at least one task description".to_string()),
            is_error: true,
        };
    }

    // Validate that every item has non-empty content after trim.
    let mut validated = Vec::with_capacity(items_to_create.len());
    for (content, priority) in items_to_create {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return ToolResult {
                call_id,
                name: "todo_create".to_string(),
                content: ToolContent::Text("content is empty".to_string()),
                is_error: true,
            };
        }
        validated.push((trimmed.to_string(), priority));
    }

    // Create the items in the store in sequential order
    let created = store.create_many(validated);
    let primary_item = created.first().cloned();

    let output = TodoCreateOutput {
        items: created,
        item: primary_item,
        total: store.len(),
    };

    ToolResult {
        call_id,
        name: "todo_create".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!(output)),
        ),
        is_error: false,
    }
}
