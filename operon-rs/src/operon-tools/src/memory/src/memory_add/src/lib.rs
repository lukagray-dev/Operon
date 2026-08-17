//! # operon-tools-memory-add
//!
//! Implements the `memory_add` tool for the Operon agent's memory group.
//!
//! Adds a new memory to the global persistent SQLite memory store.
//! Unlike session-scoped todos, memories survive process restarts and persist
//! across all sessions indefinitely until explicitly deleted.
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_memory_add::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use operon_tools_memory_store::MemoryStore;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! // let store = MemoryStore::connect_default().await.unwrap();
//! // let result = execute(
//! //     ToolCallId("call_123".to_string()),
//! //     json!({"content": "User prefers Rust over Go", "tags": ["preference"]}),
//! //     &store,
//! // ).await;
//! # }
//! ```

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::MemoryAddArgs;
pub use error::MemoryAddToolError;
pub use output::MemoryAddOutput;

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the tiered tool definition for the `memory_add` tool.
///
/// - `short`: Sent to the model normally. Concise description of the tool.
/// - `detailed`: Sent after a malformed call. Full input shapes, error cases, examples.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "content": {
                "type": "string",
                "minLength": 1,
                "description": "The memory to store — a fact, preference, or note. Use a complete sentence."
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Optional tags for categorization (e.g. [\"preference\", \"workflow\"]). Omit if not applicable."
            }
        },
        "required": ["content"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "memory_add".to_string(),
            description: "Stores a new memory persistently across all sessions. Pass `content` (required, the fact/preference to remember) and optionally `tags` (array of strings for categorization). Use this when the user states a durable preference or fact that should persist beyond this session.".to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "memory_add".to_string(),
            description: "\
Stores a new persistent memory. Memories survive restarts and persist across all sessions.

## Input shapes

`content` (required, string, non-empty): The fact/preference/note to remember. Aliases: `note`, `fact`, `text`, `memory`, `info`.

`tags` (optional, array of strings): Categorization tags. Also accepts a single string. Alias: `tag`.

## Output

```json
{ \"memory\": { \"id\": \"1\", \"content\": \"...\", \"tags\": [], \"created_at\": \"...\", \"updated_at\": \"...\" }, \"total\": 1 }
```

## Errors

- `\"content is empty\"` — provide a non-empty string
- `\"store error: ...\"` — SQLite-level failure (rare)

## Examples

```json
{\"content\": \"User prefers dark mode\", \"tags\": [\"preference\"]}
{\"note\": \"Project uses AGPL-3.0\"}
```"
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the memory_add tool.
///
/// Returns `Ok(ToolResult)` on both success and validation failures.
/// Returns `Err(MemoryAddToolError::ArgsParse)` only if the JSON shape is invalid.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
) -> Result<ToolResult, MemoryAddToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Deserializes `args_json` and executes the memory_add tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &MemoryStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, MemoryAddToolError> {
    // Parse the JSON arguments. Returns Err(ArgsParse) if the shape doesn't match.
    let args: MemoryAddArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "memory_add", None, "Storing memory"),
    );

    // Execute and wrap in Ok — executor never panics or returns hard errors.
    Ok(executor::execute(call_id, args, store).await)
}
