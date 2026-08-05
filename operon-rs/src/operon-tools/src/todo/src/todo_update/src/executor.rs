//! Executor for the todo_update tool — handles todo item updates.

use crate::args::TodoUpdateArgs;
use crate::output::TodoUpdateOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::TodoStore;

/// Executes the todo_update tool with the given arguments.
///
/// Validates that at least one field is provided for update, validates content if provided,
/// updates the item in the store, and returns the updated item.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized todo_update arguments containing id and optional fields to update.
/// - `store`: Mutable reference to the TodoStore where the item will be updated.
///
/// # Returns
/// A `ToolResult` with either success (JSON TodoUpdateOutput) or failure (Text error message).
pub async fn execute(
    call_id: ToolCallId,
    args: TodoUpdateArgs,
    store: &mut TodoStore,
) -> ToolResult {
    // Step 1: Validate that at least one field is provided for update.
    if args.content.is_none() && args.status.is_none() && args.priority.is_none() {
        return ToolResult {
            call_id,
            name: "todo_update".to_string(),
            content: ToolContent::Text(
                "no fields to update — provide at least one of: content, status, priority"
                    .to_string(),
            ),
            is_error: true,
        };
    }

    // Step 2: If content is provided, validate it's non-empty after trim.
    let validated_content = if let Some(ref c) = args.content {
        let trimmed = c.trim();
        if trimmed.is_empty() {
            return ToolResult {
                call_id,
                name: "todo_update".to_string(),
                content: ToolContent::Text("content is empty".to_string()),
                is_error: true,
            };
        }
        Some(trimmed.to_string())
    } else {
        None
    };

    // Step 3: Call store.update() with the validated fields.
    let updated = store.update(&args.id, validated_content, args.status, args.priority);

    // Step 4: If not found, return error.
    let item = match updated {
        Some(i) => i,
        None => {
            return ToolResult {
                call_id,
                name: "todo_update".to_string(),
                content: ToolContent::Text(format!("todo not found: id '{}'", args.id)),
                is_error: true,
            };
        }
    };

    // Step 5: Construct the output with the updated item.
    let output = TodoUpdateOutput { item };

    // Step 6: Return success with JSON output.
    ToolResult {
        call_id,
        name: "todo_update".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!(output)),
        ),
        is_error: false,
    }
}
