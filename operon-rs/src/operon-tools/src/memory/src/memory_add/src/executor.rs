//! Executor for the memory_add tool.

use crate::args::MemoryAddArgs;
use crate::output::MemoryAddOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_memory_store::MemoryStore;

/// Executes the memory_add tool: validates content, stores the memory, returns output.
///
/// # Arguments
/// - `call_id`: Unique identifier for this tool call (echoed back in the result).
/// - `args`: The deserialized memory_add arguments.
/// - `store`: Shared reference to the MemoryStore (no exclusive access needed — pool handles concurrency).
///
/// # Returns
/// A `ToolResult` with JSON `MemoryAddOutput` on success, or `is_error: true` Text on failure.
pub async fn execute(
    call_id: ToolCallId,
    args: MemoryAddArgs,
    store: &MemoryStore,
) -> ToolResult {
    // Step 1: Validate that content is non-empty after trimming.
    // An empty memory is meaningless and likely a model mistake.
    let trimmed = args.content.trim();
    if trimmed.is_empty() {
        return ToolResult {
            call_id,
            name: "memory_add".to_string(),
            content: ToolContent::Text("content is empty".to_string()),
            is_error: true,
        };
    }

    // Step 2: Call the store to persist the memory.
    // tags defaults to an empty Vec if not provided — valid and common.
    let tags = args.tags.unwrap_or_default();
    let memory = match store.add(trimmed.to_string(), tags).await {
        Ok(m) => m,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "memory_add".to_string(),
                content: ToolContent::Text(format!("store error: {}", e)),
                is_error: true,
            };
        }
    };

    // Step 3: Fetch the updated total count for the output.
    let total = store.count().await.unwrap_or(0);

    // Step 4: Return success with JSON output.
    let output = MemoryAddOutput { memory, total };
    ToolResult {
        call_id,
        name: "memory_add".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!({"error": "serialization failed"})),
        ),
        is_error: false,
    }
}
