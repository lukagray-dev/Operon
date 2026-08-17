//! Executor for the memory_edit tool.

use crate::args::MemoryEditArgs;
use crate::output::MemoryEditOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_memory_store::MemoryStore;

/// Executes the memory_edit tool: validates args, partially updates the memory, returns output.
pub async fn execute(
    call_id: ToolCallId,
    args: MemoryEditArgs,
    store: &MemoryStore,
) -> ToolResult {
    // Step 1: Require at least one field to update — a no-op edit is always a mistake.
    if args.content.is_none() && args.tags.is_none() {
        return ToolResult {
            call_id,
            name: "memory_edit".to_string(),
            content: ToolContent::Text(
                "no fields to update — provide at least one of: content, tags".to_string(),
            ),
            is_error: true,
        };
    }

    // Step 2: If content is provided, validate it's non-empty after trimming.
    let validated_content = if let Some(ref c) = args.content {
        let trimmed = c.trim();
        if trimmed.is_empty() {
            return ToolResult {
                call_id,
                name: "memory_edit".to_string(),
                content: ToolContent::Text("content is empty".to_string()),
                is_error: true,
            };
        }
        Some(trimmed.to_string())
    } else {
        None
    };

    // Step 3: Call the store. Returns None if id not found.
    let result = match store.edit(&args.id, validated_content, args.tags).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "memory_edit".to_string(),
                content: ToolContent::Text(format!("store error: {}", e)),
                is_error: true,
            };
        }
    };

    // Step 4: Handle not found.
    let memory = match result {
        Some(m) => m,
        None => {
            return ToolResult {
                call_id,
                name: "memory_edit".to_string(),
                content: ToolContent::Text(format!("memory not found: id '{}'", args.id)),
                is_error: true,
            };
        }
    };

    // Step 5: Return success.
    let output = MemoryEditOutput { memory };
    ToolResult {
        call_id,
        name: "memory_edit".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output)
                .unwrap_or_else(|_| serde_json::json!({"error": "serialization failed"})),
        ),
        is_error: false,
    }
}
