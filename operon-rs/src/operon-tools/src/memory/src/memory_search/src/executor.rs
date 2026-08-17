//! Executor for the memory_search tool.

use crate::args::MemorySearchArgs;
use crate::output::MemorySearchOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_memory_store::MemoryStore;

/// Executes the memory_search tool: validates query, runs FTS5, returns ranked results.
pub async fn execute(
    call_id: ToolCallId,
    args: MemorySearchArgs,
    store: &MemoryStore,
) -> ToolResult {
    // Step 1: Validate that query is non-empty.
    let trimmed = args.query.trim();
    if trimmed.is_empty() {
        return ToolResult {
            call_id,
            name: "memory_search".to_string(),
            content: ToolContent::Text("query is empty".to_string()),
            is_error: true,
        };
    }

    let limit = args.limit.unwrap_or(10);

    // Step 2: Run FTS5 search.
    let memories = match store.search(trimmed, limit).await {
        Ok(m) => m,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "memory_search".to_string(),
                content: ToolContent::Text(format!("store error: {}", e)),
                is_error: true,
            };
        }
    };

    // Step 3: Return results.
    let count = memories.len();
    let output = MemorySearchOutput {
        memories,
        count,
        query: trimmed.to_string(),
    };
    ToolResult {
        call_id,
        name: "memory_search".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output)
                .unwrap_or_else(|_| serde_json::json!({"error": "serialization failed"})),
        ),
        is_error: false,
    }
}
