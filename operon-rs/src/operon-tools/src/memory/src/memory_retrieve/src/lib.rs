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
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the tiered tool definition for the `memory_retrieve` tool.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": ["string", "integer"],
                "description": "Fetch a single memory by id. If omitted, lists all memories."
            },
            "limit": {
                "type": "integer",
                "description": "Max memories to return in list mode (default: 20)."
            },
            "offset": {
                "type": "integer",
                "description": "Number of memories to skip for pagination (default: 0)."
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "memory_retrieve".to_string(),
            description: "Fetches one memory by `id`, or lists all memories (most recent first) with `limit`/`offset` pagination. Returns `memories` array, `total`, `limit`, and `offset`.".to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "memory_retrieve".to_string(),
            description: "\
Fetches memories from the store. Two modes depending on whether `id` is provided.

## Input shapes

**Single-record mode** — `id` present:
```json
{\"id\": \"3\"}
{\"id\": 3}
```
Returns only that memory, or an error if not found.

**List mode** — no `id`:
```json
{}
{\"limit\": 10, \"offset\": 0}
{\"limit\": 5, \"offset\": 20}
```
Returns memories ordered most recent first. `limit` defaults to 20, `offset` to 0.

## Output

```json
{ \"memories\": [...], \"total\": 12, \"limit\": 20, \"offset\": 0 }
```

## Errors

- `\"memory not found: id 'X'\"` — only in single-record mode"
                .to_string(),
            parameters,
        },
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
