# operon-tools-core

Shared core types and utilities for all Operon tool crates and the tool dispatcher. This is a lightweight foundational crate with minimal dependencies.

## What Lives Here

### `ToolDefinition` Re-export

Canonical tool definitions following OpenAI, Anthropic, and Google function-calling industry specifications:
- `name: String`
- `description: String`
- `parameters: serde_json::Value` (JSON Schema object)

### `ToolProgress` & `ToolProgressEmitter`

Runtime progress event channels for streaming real-time status updates from long-running tool executions (`Started`, `Running`, `Completed`, `Failed`) to the TUI and GUI.

### `ReadLedger`

Read-before-write safety ledger that records files read during the current session and enforces that the model must inspect existing files before overwriting or editing them.

### `TodoStore` & `TodoItem`

In-memory task/todo state management used by the agent during session execution.

### `ToolDispatchError`

Error types for the tool dispatcher:
- `UnknownTool` — the model called a tool that isn't registered
- `MalformedArgs` — the model's arguments failed to deserialize into the tool schema

## Architecture

This is a **core foundational crate** with zero dependencies on other `operon-*` crates except `operon-context-normalize-tools`. Every tool sub-crate and the dispatcher depend on this.

```
operon-tools-core
  ├── ToolDefinition re-export
  ├── ReadLedger
  ├── TodoStore & TodoItem
  ├── ToolProgress & ToolProgressEmitter
  └── ToolDispatchError
```

## Usage

Tool crates return canonical `ToolDefinition` from their `definition()` function:

```rust
use operon_context_normalize_tools::ToolDefinition;
use serde_json::json;

pub fn definition() -> ToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Target file path." }
        },
        "required": ["path"]
    });

    ToolDefinition {
        name: "my_tool".to_string(),
        description: "Concise, industry-standard description of the tool.".to_string(),
        parameters,
    }
}
```

## Design Constraints

- **Minimal deps**: `thiserror`, `serde`, `serde_json`, `tokio::sync::mpsc`, and `operon-context-normalize-tools`
- **No `unwrap()` or `expect()`** in production code

## License

AGPL-3.0 — See the workspace root for details.
