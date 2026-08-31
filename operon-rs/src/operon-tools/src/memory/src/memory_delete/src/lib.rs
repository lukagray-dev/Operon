//! # operon-tools-memory-delete
//!
//! Implements the `memory_delete` tool — permanently removes a memory by id.

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::MemoryDeleteArgs;
pub use error::MemoryDeleteToolError;
pub use output::MemoryDeleteOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the canonical tool definition for the `memory_delete` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`id`).
/// - Clear parameter description for memory ID.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the parameters schema for deleting memories here.
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": ["string", "integer"],
                "description": "ID of the memory to permanently delete."
            }
        },
        "required": ["id"]
    });

    ToolDefinition {
        name: "memory_delete".to_string(),
        description: "Permanently deletes a memory by `id`. Returns the deleted id and remaining count. This action is irreversible — use `memory_retrieve` to confirm the id first.".to_string(),
        parameters,
    }
}

/// Deserializes `args_json` and executes the memory_delete tool.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
) -> Result<ToolResult, MemoryDeleteToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the memory_delete tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, MemoryDeleteToolError> {
    let args: MemoryDeleteArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "memory_delete", None, "Deleting memory"),
    );

    Ok(executor::execute(call_id, args, store).await)
}
