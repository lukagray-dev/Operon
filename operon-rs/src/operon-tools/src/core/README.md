# operon-tools-core

Shared core types for all Operon tool crates. This is a pure types crate with no I/O, no async, and minimal dependencies.

## What Lives Here

### `TieredToolDefinition`

A tool definition with two description tiers: **short** and **detailed**.

- **Short description**: Used under normal conditions. Concise (≤5 lines), states what the tool does and key constraints.
- **Detailed description**: Used after the model makes a malformed call. Full explanation with input shapes, edge cases, worked examples, and common mistakes.

The dispatcher automatically switches from short to detailed when a tool receives malformed arguments, helping the model recover gracefully.

### `ToolDispatchError`

Error types for the tool dispatcher:

- `UnknownTool` — the model called a tool that isn't registered
- `MalformedArgs` — the model's arguments failed to deserialize (triggers degradation)
- `InternalError` — unexpected runtime error in the tool implementation

## Architecture

This is a **leaf crate** with zero dependencies on other `operon-*` crates except `operon-context-normalize-tools`. Every tool sub-crate and the dispatcher depend on this.

```
operon-tools-core
  ├── No async runtime
  ├── No I/O
  ├── No operon-context pipeline deps
  └── Pure types + thiserror + serde
```

## Usage

Tool crates return `TieredToolDefinition` from their `definition()` function:

```rust
use operon_tools_core::TieredToolDefinition;
use operon_context_normalize_tools::ToolDefinition;
use serde_json::json;

pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" }
        },
        "required": ["path"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "my_tool".to_string(),
            description: "Does something useful. Max 5 lines.".to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "my_tool".to_string(),
            description: "Full explanation with examples and edge cases...".to_string(),
            parameters,
        },
    }
}
```

## Invariants

- `short.name` and `detailed.name` **MUST** be identical
- `short.parameters` and `detailed.parameters` **MUST** be identical (same JSON Schema)
- Only `description` differs between the two tiers

A `debug_assert!` in `TieredToolDefinition::for_mode()` enforces the name invariant in debug builds.

## When Each Tier Is Used

1. **Session start**: All tools use `short` descriptions
2. **Malformed call**: If a tool receives invalid arguments, it's marked "degraded"
3. **Next request**: Degraded tools use `detailed` descriptions
4. **Session reset**: All tools revert to `short`

This allows the model to see more context only when it needs help, keeping the normal-case token usage low.

## Design Constraints

- **Sync-only**: No `tokio`, no `async_trait`, no async functions
- **No I/O**: No file system, no network, no process spawning
- **Minimal deps**: Only `thiserror`, `serde`, `serde_json`, and `operon-context-normalize-tools`
- **No `unwrap()` or `expect()`** in production code

## License

AGPL-3.0 — See the workspace root for details.
