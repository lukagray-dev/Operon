# operon-tools

All Operon agent tool groups and their runtime dispatcher. This crate provides the central tool execution infrastructure for the Operon agent.

## Architecture

```
operon-tools/
  ├── dispatcher/       — Routes tool calls, enforces read-before-write safety, progress events
  │     ├── mod.rs        — Dispatcher struct, lifecycle methods, definitions iterator
  │     ├── register.rs   — Group registration methods (fs, shell, web, todo, memory, ask)
  │     ├── dispatch.rs   — Execution pipeline, stateful tool routing (todo/memory), ledger checks
  │     └── ledger.rs     — Read ledger recording helpers
  ├── fs/               — Filesystem tools (read, grep, glob, ls, write, edit, append, delete)
  ├── shell/            — Shell execution tools (bash)
  ├── web/              — Web search and fetch tools (web_search, web_fetch)
  ├── todo/             — Session todo management tools (create, list, update, delete)
  ├── memory/           — Persistent SQLite memory tools (add, edit, delete, retrieve, search)
  └── ask/              — User confirmation and interactive choice tool
```

## The Dispatcher

The `Dispatcher` is the central component that:

1. **Exposes all 21 tools upfront**: All tools and their canonical JSON Schemas are available from turn 1.
2. **Routes tool calls**: Dispatches model tool calls to their respective implementations.
3. **Enforces Read-Before-Write Safety**: Guarantees that existing files must be read before being edited or overwritten via `ReadLedger`.
4. **Streams Real-Time Progress Events**: Emits granular lifecycle progress updates (`Started`, `Running`, `Completed`, `Failed`) to the TUI and GUI.
5. **Handles errors gracefully**: Converts errors to `ToolResult` so the model can inspect error details and self-correct.

### Session Lifecycle

Create one `Dispatcher` per agent session:

```rust
use operon_tools::dispatcher::Dispatcher;

// Session start
let mut dispatcher = Dispatcher::new();
dispatcher.register_fs_tools();
dispatcher.register_shell_tools();
dispatcher.register_web_tools();
dispatcher.register_todo_tools();
dispatcher.register_ask_tool();
dispatcher.register_memory_tools();

// Get definitions to send to the model
let defs: Vec<_> = dispatcher.definitions().collect();

// After the model calls a tool
let result = dispatcher.dispatch(tool_call).await;
```

## Dispatch Flow

```text
ToolCall from model
  ↓
dispatcher.dispatch(call)
  ↓
Intercept stateful tools (Todo / Memory)
  ├─ todo_*   → Handled via &mut dispatcher.todo_store
  └─ memory_* → Handled via &dispatcher.memory_store
  ↓
Lookup standard tool by name
  ├─ Not found → UnknownTool error ToolResult
  └─ Found
      ↓
Read-before-write / Read-before-edit verification
      ↓
Execute tool with progress reporting
  ├─ Parse failure → MalformedArgs error ToolResult
  └─ Success       → ToolResult (record read path into ReadLedger if tool is `read`)
```

## Tool Groups

Operon includes 21 built-in agent tools:

1. **Filesystem (`fs`)** (8 tools):
   - `read`: Reads one or multiple files in batch with inline line ranges (`"path": ["src/a.rs:10-50", "src/b.rs"]`).
   - `grep`: Regex search across files/directories with gitignore support and context lines.
   - `glob`: Fast wildcard path search (`"pattern": "**/*.rs"`) respecting `.gitignore`.
   - `ls`: Single-level directory listing with metadata and glob filters.
   - `write`: Creates or overwrites files with atomic write guarantees.
   - `edit`: Precise text hunk replacements with 6-pass fuzzy matching.
   - `append`: Appends text to existing files non-destructively.
   - `delete`: Safely removes files/directories (trash or permanent).
2. **Shell (`shell`)** (1 tool):
   - `bash`: Cross-platform command execution with timeout and output capture.
3. **Web (`web`)** (2 tools):
   - `web_search`: Live search query across the web.
   - `web_fetch`: URL fetching and HTML-to-markdown extraction.
4. **Todo (`todo`)** (4 tools):
   - `todo_create`, `todo_list`, `todo_update`, `todo_delete`.
5. **Memory (`memory`)** (5 tools):
   - `memory_add`, `memory_edit`, `memory_delete`, `memory_retrieve`, `memory_search`.
6. **Ask (`ask`)** (1 tool):
   - Interactive user choice and confirmation prompts.

## Testing

```bash
cargo test -p operon-tools
```

## License

AGPL-3.0 — See the workspace root for details.
