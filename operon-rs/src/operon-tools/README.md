# operon-tools

All Operon agent tool groups and their runtime dispatcher. This crate provides the central tool execution infrastructure for the Operon agent.

## Architecture

```
operon-tools/
  ├── dispatcher.rs     — Routes tool calls, manages tiered descriptions per session
  ├── fs/               — Filesystem tools (read, write, edit, grep, ...)
  ├── web/              — Web tools (fetch, search, ...) [future]
  └── sub_agents/       — Sub-agent tools [future]
```

## The Dispatcher

The `Dispatcher` is the central component that:

1. **Routes tool calls** from the model to the correct implementation
2. **Manages tiered descriptions** — switches from short to detailed when a tool receives malformed args
3. **Handles errors gracefully** — converts all errors to `ToolResult` so the model can see what went wrong

### Session Lifecycle

Create one `Dispatcher` per agent session:

```rust
use operon_tools::dispatcher::Dispatcher;

// Session start
let mut dispatcher = Dispatcher::new();
dispatcher.register_fs_tools();

// Get definitions to send to the model
let defs: Vec<_> = dispatcher.definitions().collect();

// After the model calls a tool
let result = dispatcher.dispatch(tool_call).await;

// Session end — drop the dispatcher
```

A new session gets a fresh dispatcher with all tools back in short-description mode.

## Tiered Descriptions

The dispatcher implements a **graceful degradation** strategy:

### Normal Operation (Short Descriptions)

```
Model sees: "read — Reads files (max 1 MB). Pass paths as array."
Model calls: read with valid args
Result: Success
```

### After Malformed Call (Detailed Descriptions)

```
Model sees: "read — Reads files (max 1 MB). Pass paths as array."
Model calls: read with { "path": "file.txt" }  ← wrong! should be "paths" (array)
Result: Error ToolResult explaining the mistake
Dispatcher marks "read" as degraded

Next request:
Model sees: "read — Full explanation with examples, edge cases, common mistakes..."
Model calls: read with { "paths": ["file.txt"] }  ← correct!
Result: Success
```

The detailed description stays active for the rest of the session. Other tools remain in short mode.

## Dispatch Flow

```text
ToolCall from model
  ↓
dispatcher.dispatch(call)
  ↓
Look up tool by name
  ├─ Not found → UnknownTool error ToolResult
  └─ Found
      ↓
Parse arguments
  ├─ Parse failure → MalformedArgs error ToolResult + mark tool degraded
  └─ Parse success
      ↓
Execute tool
  ├─ Runtime error → InternalError error ToolResult
  └─ Success → ToolResult (may contain per-file/per-item errors in JSON)
```

**Key property**: `dispatch()` always returns a `ToolResult`, never propagates errors. The model sees all failures as structured tool results.

## Registering Tools

### Filesystem Tools

```rust
dispatcher.register_fs_tools();
```

This registers all tools from the `fs` group:
- `read` — reads one or multiple files with optional line ranges

### Custom Tools (Future)

```rust
dispatcher.register(
    my_tool::definition(),
    |call_id, args| async move {
        my_tool::execute(call_id, args)
            .await
            .map_err(|e| e.to_string())
    },
);
```

The execute closure must:
- Return `Err(String)` **only** for args parse failures (triggers degradation)
- Return `Ok(ToolResult { is_error: true, ... })` for runtime errors (file not found, network timeout, etc.)

## Error Handling Philosophy

### Top-Level Errors (Rare)

These become error `ToolResult` with `is_error: true`:

- Unknown tool name
- Malformed arguments (also triggers degradation)
- Internal tool runtime bugs

### Per-Item Errors (Common)

These are embedded in the `ToolResult` JSON content with `is_error: false`:

- File not found (in a multi-file read)
- Network timeout (in a batch fetch)
- Permission denied (in a file write)

The tool call itself succeeded — it processed the request and returned structured results. Individual item failures are part of the normal response.

## Testing

The dispatcher has comprehensive tests covering:

- Unknown tool handling
- Malformed args detection and degradation
- Tiered description switching
- Successful dispatch without degradation
- Isolation (degrading one tool doesn't affect others)

Run tests:

```bash
cargo test -p operon-tools
```

## Tool Groups

### `fs` — Filesystem Tools

- **`read`**: Reads one or multiple files in a single call. Supports line ranges for large files, binary detection, 1 MB size limit on full-file reads.

Future groups:
- **`web`**: HTTP fetch, web search, scraping
- **`sub_agents`**: Spawn and manage sub-agents
- **`git`**: Repository operations
- **`shell`**: Safe command execution

## Design Constraints

- **No `unwrap()` or `expect()`** in production code (except the justified semaphore acquire in `executor.rs`)
- **Dispatcher is session-scoped** — one instance per agent session, dropped on session end
- **Tool crates are stateless** — all state lives in the dispatcher
- **Errors never propagate** — `dispatch()` always returns `ToolResult`

## Dependencies

- `operon-tools-core` — shared types (`TieredToolDefinition`, `ToolDispatchError`)
- `operon-tools-fs` — filesystem tool group facade
- `operon-tools-fs-read` — read tool implementation
- `operon-context-normalize-tools` — canonical tool types (`ToolCall`, `ToolResult`, etc.)
- `tokio` — async runtime
- `serde_json` — JSON handling

## License

AGPL-3.0 — See the workspace root for details.
