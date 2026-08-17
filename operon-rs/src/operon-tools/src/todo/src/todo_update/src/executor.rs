//! Executor for the todo_update tool — handles single and batch todo item updates.

use crate::args::TodoUpdateArgs;
use crate::output::TodoUpdateOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::TodoStore;

/// Executes the todo_update tool with the given arguments.
pub async fn execute(
    call_id: ToolCallId,
    args: TodoUpdateArgs,
    store: &mut TodoStore,
) -> ToolResult {
    let raw_updates = args.into_updates();

    if raw_updates.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_update".to_string(),
            content: ToolContent::Text(
                "no target tasks specified — provide `id`, `ids`, or `todos`".to_string(),
            ),
            is_error: true,
        };
    }

    let is_single = raw_updates.len() == 1;
    let mut validated_updates = Vec::with_capacity(raw_updates.len());

    for (id, content, status, priority) in raw_updates {
        // Must provide at least one field to update
        if content.is_none() && status.is_none() && priority.is_none() {
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

        // If content is provided, validate it is non-empty
        let validated_content = if let Some(c) = content {
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

        validated_updates.push((id, validated_content, status, priority));
    }

    // Apply batch updates in the store
    let (updated_items, not_found_ids) = store.update_many(validated_updates);

    // If single update failed because item wasn't found, return standard error
    if is_single && updated_items.is_empty() && !not_found_ids.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_update".to_string(),
            content: ToolContent::Text(format!("todo not found: id '{}'", not_found_ids[0])),
            is_error: true,
        };
    }

    // If batch update produced no successes and all failed:
    if updated_items.is_empty() && !not_found_ids.is_empty() {
        return ToolResult {
            call_id,
            name: "todo_update".to_string(),
            content: ToolContent::Text(format!(
                "todos not found: IDs [{}]",
                not_found_ids.join(", ")
            )),
            is_error: true,
        };
    }

    let primary_item = updated_items.first().cloned();
    let output = TodoUpdateOutput {
        items: updated_items,
        item: primary_item,
        not_found: not_found_ids,
    };

    ToolResult {
        call_id,
        name: "todo_update".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!(output)),
        ),
        is_error: false,
    }
}
