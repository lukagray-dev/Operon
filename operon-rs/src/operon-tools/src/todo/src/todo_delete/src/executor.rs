//! Executor for the todo_delete tool — handles single and batch todo item deletions.

use crate::args::TodoDeleteArgs;
use crate::output::TodoDeleteOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::TodoStore;

/// Executes the todo_delete tool with the given arguments.
pub async fn execute(
    call_id: ToolCallId,
    args: TodoDeleteArgs,
    store: &mut TodoStore,
) -> ToolResult {
    let ids_to_delete = args.into_ids();

    if ids_to_delete.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_delete".to_string(),
            content: ToolContent::Text(
                "no target tasks specified — provide `id`, `ids`, or `todos`".to_string(),
            ),
            is_error: true,
        };
    }

    let is_single = ids_to_delete.len() == 1;
    let (deleted_ids, not_found_ids) = store.delete_many(&ids_to_delete);

    // If single delete failed because item wasn't found, return standard error message
    if is_single && deleted_ids.is_empty() && !not_found_ids.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_delete".to_string(),
            content: ToolContent::Text(format!("todo not found: id '{}'", not_found_ids[0])),
            is_error: true,
        };
    }

    // If all deletions failed:
    if deleted_ids.is_empty() && !not_found_ids.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_delete".to_string(),
            content: ToolContent::Text(format!(
                "todos not found: IDs [{}]",
                not_found_ids.join(", ")
            )),
            is_error: true,
        };
    }

    let primary_id = deleted_ids.first().cloned();
    let output = TodoDeleteOutput {
        ids: deleted_ids,
        id: primary_id,
        not_found: not_found_ids,
        remaining: store.len(),
    };

    ToolResult {
        call_id,
        name: "todo_delete".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!(output)),
        ),
        is_error: false,
    }
}
