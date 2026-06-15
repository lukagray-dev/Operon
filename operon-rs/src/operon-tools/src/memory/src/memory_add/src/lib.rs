//! # operon-tools-memory-add
//!
//! Entry point for the memory_add tool, exposing definition and execute functions.

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::MemoryAddArgs;
pub use error::MemoryAddToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `memory_add` tool.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "memory_add".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Parses args and executes the memory_add tool.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, MemoryAddToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses args and executes the memory_add tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, MemoryAddToolError> {
    let args = MemoryAddArgs::parse(&args_json).map_err(MemoryAddToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "memory_add", None, "Adding memory"),
    );

    executor::execute(call_id, args).await
}
