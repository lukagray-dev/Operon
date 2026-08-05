//! Executor for the todo_delete tool — handles todo item deletion.

use crate::args::TodoDeleteArgs;
use crate::output::TodoDeleteOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::TodoStore;

/// Executes the todo_delete tool with the given arguments.
///
/// Deletes the todo item with the specified ID from the store and returns
/// the deleted ID and the remaining count.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized todo_delete arguments containing the id to delete.
/// - `store`: Mutable reference to the TodoStore where the item will be deleted.
///
/// # Returns
/// A `ToolResult` with either success (JSON TodoDeleteOutput) or failure (Text error message).
pub async fn execute(
    call_id: ToolCallId,
    args: TodoDeleteArgs,
    store: &mut TodoStore,
) -> ToolResult {
    // Step 1: Attempt to delete the item from the store.
    let deleted = store.delete(&args.id);

    // Step 2: If not found, return error.
    if !deleted {
        return ToolResult {
            call_id,
            name: "todo_delete".to_string(),
            content: ToolContent::Text(format!("todo not found: id '{}'", args.id)),
            is_error: true,
        };
    }

    // Step 3: Construct the output with the deleted id and remaining count.
    let output = TodoDeleteOutput {
        id: args.id,
        remaining: store.len(),
    };

    // Step 4: Return success with JSON output.
    ToolResult {
        call_id,
        name: "todo_delete".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!(output)),
        ),
        is_error: false,
    }
}
