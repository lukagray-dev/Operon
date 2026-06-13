//! Executor for the todo_update tool — handles todo item updates.

use crate::args::TodoUpdateArgs;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};

use operon_tools_core::TodoStore;

/// Executes the todo_update tool with the given arguments.
///
/// Validates that at least one field is provided for update, validates todo if provided,
/// updates the item in the store, and returns the updated item in plain text.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The parsed todo_update arguments containing id and optional fields to update.
/// - `store`: Mutable reference to the TodoStore where the item will be updated.
///
/// # Returns
/// A `ToolResult` with plain-text content (ToolContent::Text).
pub async fn execute(
    call_id: ToolCallId,
    args: TodoUpdateArgs,
    store: &mut TodoStore,
) -> ToolResult {
    // Step 1: Validate that at least one field is provided for update.
    if args.todo.is_none() && args.status.is_none() && args.priority.is_none() {
        return ToolResult {
            call_id,
            name: "todo_update".to_string(),
            content: ToolContent::Text(
                "no fields to update — provide at least one of: todo, status, priority"
                    .to_string(),
            ),
            is_error: true,
            read_paths: None,
        };
    }

    // Step 2: If todo is provided, validate it's non-empty after trim.
    let validated_todo = if let Some(ref c) = args.todo {
        let trimmed = c.trim();
        if trimmed.is_empty() {
            return ToolResult {
                call_id,
                name: "todo_update".to_string(),
                content: ToolContent::Text("todo is empty".to_string()),
                is_error: true,
                read_paths: None,
            };
        }
        Some(trimmed.to_string())
    } else {
        None
    };

    // Step 3: Call store.update() with the validated fields.
    let updated = store.update(&args.id, validated_todo, args.status, args.priority);

    // Step 4: If not found, return error.
    let item = match updated {
        Some(i) => i,
        None => {
            return ToolResult {
                call_id,
                name: "todo_update".to_string(),
                content: ToolContent::Text(format!("todo not found: id '{}'", args.id)),
                is_error: true,
                read_paths: None,
            };
        }
    };

    // Step 5: Format output as plain text.
    let text = format!(
        "Updated #{}: {} [{}, {}]",
        item.id, item.content, item.status, item.priority
    );

    // Step 6: Return success with plain-text output.
    ToolResult {
        call_id,
        name: "todo_update".to_string(),
        content: ToolContent::Text(text),
        is_error: false,
        read_paths: None,
    }
}
