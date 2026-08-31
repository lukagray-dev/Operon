//! # operon-tools-memory-edit
//!
//! Implements the `memory_edit` tool — partial update of an existing memory.

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::MemoryEditArgs;
pub use error::MemoryEditToolError;
pub use output::MemoryEditOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the canonical tool definition for the `memory_edit` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`id`).
/// - Clear parameter descriptions for id, updated content, and updated tags.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the parameters schema for editing existing memories here.
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": ["string", "integer"],
                "description": "ID of the memory to update."
            },
            "content": {
                "type": "string",
                "description": "New content for the memory."
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "New tags (replaces current tags)."
            }
        },
        "required": ["id"]
    });

    ToolDefinition {
        name: "memory_edit".to_string(),
        description: "Partially updates an existing memory. Pass `id` (required) and any of: `content`, `tags`. Only provided fields change; omitted fields are unchanged.".to_string(),
        parameters,
    }
}

/// Deserializes `args_json` and executes the memory_edit tool.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
) -> Result<ToolResult, MemoryEditToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the memory_edit tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, MemoryEditToolError> {
    let args: MemoryEditArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "memory_edit", None, "Updating memory"),
    );

    Ok(executor::execute(call_id, args, store).await)
}
