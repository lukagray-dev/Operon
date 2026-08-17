//! Executor for the memory_delete tool.

use crate::args::MemoryDeleteArgs;
use crate::output::MemoryDeleteOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_memory_store::MemoryStore;

/// Executes the memory_delete tool: deletes the memory, returns the deleted id and remaining count.
pub async fn execute(
    call_id: ToolCallId,
    args: MemoryDeleteArgs,
    store: &MemoryStore,
) -> ToolResult {
    // Step 1: Attempt deletion. Returns false if id not found.
    let deleted = match store.delete(&args.id).await {
        Ok(d) => d,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "memory_delete".to_string(),
                content: ToolContent::Text(format!("store error: {}", e)),
                is_error: true,
            };
        }
    };

    // Step 2: Not found → error.
    if !deleted {
        return ToolResult {
            call_id,
            name: "memory_delete".to_string(),
            content: ToolContent::Text(format!("memory not found: id '{}'", args.id)),
            is_error: true,
        };
    }

    // Step 3: Fetch remaining count.
    let remaining = store.count().await.unwrap_or(0);

    // Step 4: Return success.
    let output = MemoryDeleteOutput { id: args.id, remaining };
    ToolResult {
        call_id,
        name: "memory_delete".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output)
                .unwrap_or_else(|_| serde_json::json!({"error": "serialization failed"})),
        ),
        is_error: false,
    }
}
