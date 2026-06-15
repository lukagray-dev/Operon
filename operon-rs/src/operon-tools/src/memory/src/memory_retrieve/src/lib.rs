//! # operon-tools-memory-retrieve
//!
//! Entry point for the memory_retrieve tool.

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::MemoryRetrieveArgs;
pub use error::MemoryRetrieveToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `memory_retrieve` tool.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "memory_retrieve".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses args and executes the memory_retrieve tool.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, MemoryRetrieveToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses args and executes the memory_retrieve tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, MemoryRetrieveToolError> {
    let args = MemoryRetrieveArgs::parse(&args_json).map_err(MemoryRetrieveToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "memory_retrieve", None, "Retrieving memory"),
    );

    executor::execute(call_id, args).await
}
