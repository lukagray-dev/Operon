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
use operon_tools_core::{emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the tiered tool definition for the `memory_delete` tool.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": ["string", "integer"],
                "description": "Id of the memory to permanently delete."
            }
        },
        "required": ["id"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "memory_delete".to_string(),
            description: "Permanently deletes a memory by `id`. Returns the deleted id and remaining count. This action is irreversible — use `memory_retrieve` to confirm the id first.".to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "memory_delete".to_string(),
            description: "\
Permanently deletes a single memory. This action is irreversible.

## Input shapes

`id` (required, string or integer): Id of the memory to delete. Aliases: `memory_id`, `memoryId`.

## Output

```json
{ \"id\": \"1\", \"remaining\": 4 }
```

## Errors

- `\"memory not found: id 'X'\"` — no memory with that id exists
- `\"store error: ...\"` — SQLite-level failure (rare)

## Example

```json
{\"id\": \"3\"}
{\"id\": 5}
```"
                .to_string(),
            parameters,
        },
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
