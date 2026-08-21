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
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Returns the tiered tool definition for the `memory_search` tool.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "minLength": 1,
                "description": "FTS5 search query. Aliases: q, text, term, terms."
            },
            "limit": {
                "type": "integer",
                "description": "Max results to return, ranked by relevance (default: 10)."
            }
        },
        "required": ["query"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "memory_search".to_string(),
            description: "Full-text searches memory content using FTS5 (BM25 relevance). Pass `query` (required) and optionally `limit`. Returns `memories` ranked by relevance and `count`.".to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "memory_search".to_string(),
            description: "\
Full-text searches memory content using SQLite FTS5 (BM25 relevance ranking).

## Input shapes

`query` (required, string, non-empty): Search terms. FTS5 MATCH syntax is supported:
- Single term: `\"Rust\"`
- AND: `\"Rust AND programming\"`  (implicit AND: `\"Rust programming\"`)
- OR: `\"Rust OR Go\"`
- Phrase: `\"dark mode\"`

Aliases: `q`, `text`, `term`, `terms`.

`limit` (optional, integer): Max results, ranked by relevance (default: 10).

## Output

```json
{ \"memories\": [...], \"count\": 2, \"query\": \"Rust\" }
```

## Errors

- `\"query is empty\"` — provide a non-empty search term

## Examples

```json
{\"query\": \"dark mode\"}
{\"q\": \"Rust\", \"limit\": 5}
{\"query\": \"workflow AND git\"}
```"
                .to_string(),
            parameters,
        },
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
