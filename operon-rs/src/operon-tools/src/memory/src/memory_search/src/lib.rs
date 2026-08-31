//! # operon-tools-memory-search
//!
//! Implements the `memory_search` tool — FTS5 full-text search over memory content.

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::MemorySearchArgs;
pub use error::MemorySearchToolError;
pub use output::MemorySearchOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the canonical tool definition for the `memory_search` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`query`).
/// - Clear parameter descriptions for search query syntax and limit.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the parameters schema for full-text searching memories here.
    let parameters = json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "minLength": 1,
                "description": "Full-text search query string (supports FTS5 matching)."
            },
            "limit": {
                "type": "integer",
                "description": "Maximum number of results to return, ranked by relevance (default: 10)."
            }
        },
        "required": ["query"]
    });

    ToolDefinition {
        name: "memory_search".to_string(),
        description: "Full-text searches memory content using FTS5 (BM25 relevance). Pass `query` (required) and optionally `limit`. Returns `memories` ranked by relevance and `count`.".to_string(),
        parameters,
    }
}

/// Deserializes `args_json` and executes the memory_search tool.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
) -> Result<ToolResult, MemorySearchToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the memory_search tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, MemorySearchToolError> {
    let args: MemorySearchArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "memory_search", None, "Searching memories"),
    );

    Ok(executor::execute(call_id, args, store).await)
}
