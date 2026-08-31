//! # operon-tools-memory-retrieve
//!
//! Implements the `memory_retrieve` tool — fetch one memory by id, or list all with pagination.

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::MemoryRetrieveArgs;
pub use error::MemoryRetrieveToolError;
pub use output::MemoryRetrieveOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the canonical tool definition for the `memory_retrieve` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Clear parameter descriptions for single memory lookup (`id`) and paginated listing (`limit`, `offset`).
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the parameters schema for retrieving memories here.
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": ["string", "integer"],
                "description": "Fetch a single memory by ID. If omitted, lists all memories."
            },
            "limit": {
                "type": "integer",
                "description": "Maximum number of memories to return in list mode (default: 20)."
            },
            "offset": {
                "type": "integer",
                "description": "Number of memories to skip for pagination (default: 0)."
            }
        }
    });

    ToolDefinition {
        name: "memory_retrieve".to_string(),
        description: "Fetches one memory by `id`, or lists all memories (most recent first) with `limit`/`offset` pagination. Returns `memories` array, `total`, `limit`, and `offset`.".to_string(),
        parameters,
    }
}

/// Deserializes `args_json` and executes the memory_retrieve tool.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
) -> Result<ToolResult, MemoryRetrieveToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the memory_retrieve tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, MemoryRetrieveToolError> {
    let args: MemoryRetrieveArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "memory_retrieve",
            None,
            "Retrieving memories",
        ),
    );

    Ok(executor::execute(call_id, args, store).await)
}
