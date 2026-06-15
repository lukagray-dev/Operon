//! # operon-tools-memory-edit
//!
//! Entry point for the memory_edit tool.

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::MemoryEditArgs;
pub use error::MemoryEditToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `memory_edit` tool.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "memory_edit".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses args and executes the memory_edit tool.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, MemoryEditToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses args and executes the memory_edit tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, MemoryEditToolError> {
    let args = MemoryEditArgs::parse(&args_json).map_err(MemoryEditToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "memory_edit", None, "Editing memory"),
    );

    executor::execute(call_id, args).await
}
