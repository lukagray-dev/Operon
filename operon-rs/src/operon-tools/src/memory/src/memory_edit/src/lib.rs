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
use operon_tools_core::{emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the tiered tool definition for the `memory_edit` tool.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": ["string", "integer"],
                "description": "Id of the memory to update."
            },
            "content": {
                "type": "string",
                "description": "New content for the memory. Aliases: note, fact, text, memory, info."
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "New tags (replaces current tags). Alias: tag."
            }
        },
        "required": ["id"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "memory_edit".to_string(),
            description: "Partially updates an existing memory. Pass `id` (required) and any of: `content`, `tags`. Only provided fields change; omitted fields are unchanged.".to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "memory_edit".to_string(),
            description: "\
Partially updates an existing memory. Only provided fields are changed — omitted fields remain as-is.

## Input shapes

`id` (required, string or integer): Id of the memory to update. Aliases: `memory_id`, `memoryId`.

`content` (optional, string, non-empty): New content. Aliases: `note`, `fact`, `text`, `memory`, `info`.

`tags` (optional, array of strings): Replaces all current tags. Single string also accepted. Alias: `tag`.

At least one of `content` or `tags` must be provided.

## Output

```json
{ \"memory\": { \"id\": \"1\", \"content\": \"...\", \"tags\": [], \"created_at\": \"...\", \"updated_at\": \"...\" } }
```

## Errors

- `\"no fields to update — provide at least one of: content, tags\"`
- `\"content is empty\"`
- `\"memory not found: id 'X'\"`

## Examples

```json
{\"id\": \"1\", \"content\": \"Updated preference text\"}
{\"id\": 2, \"tags\": [\"workflow\", \"git\"]}
{\"id\": \"3\", \"content\": \"New fact\", \"tags\": [\"fact\"]}
```"
                .to_string(),
            parameters,
        },
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
