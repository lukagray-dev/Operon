//! Executor for the memory_retrieve tool.

use crate::args::MemoryRetrieveArgs;
use crate::output::MemoryRetrieveOutput;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_memory_store::MemoryStore;

/// Executes the memory_retrieve tool.
/// - Single-record mode: if `id` is set, fetch just that memory.
/// - List mode: paginate through all memories (most recent first).
pub async fn execute(
    call_id: ToolCallId,
    args: MemoryRetrieveArgs,
    store: &MemoryStore,
) -> ToolResult {
    let total = store.count().await.unwrap_or(0);

    // ── Single-record mode ────────────────────────────────────────────────────
    if let Some(ref id) = args.id {
        let result = match store.get(id).await {
            Ok(r) => r,
            Err(e) => {
                return ToolResult {
                    call_id,
                    name: "memory_retrieve".to_string(),
                    content: ToolContent::Text(format!("store error: {}", e)),
                    is_error: true,
                };
            }
        };

        let memory = match result {
            Some(m) => m,
            None => {
                return ToolResult {
                    call_id,
                    name: "memory_retrieve".to_string(),
                    content: ToolContent::Text(format!("memory not found: id '{}'", id)),
                    is_error: true,
                };
            }
        };

        let output = MemoryRetrieveOutput {
            memories: vec![memory],
            total,
            offset: 0,
            limit: 1,
        };
        return ToolResult {
            call_id,
            name: "memory_retrieve".to_string(),
            content: ToolContent::Json(
                serde_json::to_value(&output)
                    .unwrap_or_else(|_| serde_json::json!({"error": "serialization failed"})),
            ),
            is_error: false,
        };
    }

    // ── List mode ─────────────────────────────────────────────────────────────
    let limit = args.limit.unwrap_or(20);
    let offset = args.offset.unwrap_or(0);

    let memories = match store.list(limit, offset).await {
        Ok(m) => m,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "memory_retrieve".to_string(),
                content: ToolContent::Text(format!("store error: {}", e)),
                is_error: true,
            };
        }
    };

    let output = MemoryRetrieveOutput {
        memories,
        total,
        offset,
        limit,
    };
    ToolResult {
        call_id,
        name: "memory_retrieve".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output)
                .unwrap_or_else(|_| serde_json::json!({"error": "serialization failed"})),
        ),
        is_error: false,
    }
}
