//! Executor for the todo_delete tool — handles todo item deletion.

use crate::args::TodoDeleteArgs;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};

use operon_tools_core::TodoStore;

/// Executes the todo_delete tool with the given arguments.
///
/// Deletes the todo item with the specified ID from the store and returns
/// confirmation text.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The parsed todo_delete arguments containing the id to delete.
/// - `store`: Mutable reference to the TodoStore where the item will be deleted.
///
/// # Returns
/// A `ToolResult` with plain-text content (ToolContent::Text).
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
            read_paths: None,
        };
    }

    // Step 3: Format confirmation message.
    let text = format!("Deleted #{}. {} todo(s) remaining.", args.id, store.len());

    // Step 4: Return success with plain-text output.
    ToolResult {
        call_id,
        name: "todo_delete".to_string(),
        content: ToolContent::Text(text),
        is_error: false,
        read_paths: None,
    }
}
