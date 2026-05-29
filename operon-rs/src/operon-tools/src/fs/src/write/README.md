# operon-tools-fs-write

The `write` tool for Operon's filesystem group. Creates new files or completely overwrites existing files with atomic writes.

## Overview

`operon-tools-fs-write` is a production-grade filesystem tool that safely writes file content to disk. It's designed for:
- Creating new files from scratch
- Completely replacing existing file content
- Ensuring atomic writes (all-or-nothing semantics)
- Validating that parent directories exist before writing

## Key Features

### Atomic Writes
Files are written atomically using a temp file + rename pattern. If the write fails at any point (disk full, permission denied, etc.), the original file remains untouched.

### Create vs Overwrite Detection
The tool automatically detects whether it's creating a new file or overwriting an existing one, and reports this in the output via the `created` field.

### Parent Directory Validation
The tool validates that the parent directory exists before attempting to write. It does **not** create intermediate directories — this is a separate concern handled by other tools (e.g., bash).

### UTF-8 Aware
Byte counts are accurate for UTF-8 content, including multi-byte characters (e.g., "héllo" = 6 bytes, not 5).

### Comprehensive Error Handling
Clear, actionable error messages for all failure cases:
- Parent directory doesn't exist
- Temp file write failed
- Atomic rename failed

## Architecture

### Module Structure

```
src/
├── lib.rs          # Public API: definition(), execute()
├── args.rs         # WriteArgs struct (path, content)
├── error.rs        # WriteToolError enum
├── output.rs       # WriteOutput struct (success response)
├── executor.rs     # Core write logic with atomic pattern
└── tests.rs        # 17 comprehensive tests
```

### Execution Flow

1. **Argument Parsing** — Deserialize JSON into `WriteArgs`
2. **Parent Validation** — Check parent directory exists
3. **State Detection** — Determine if this is create or overwrite
4. **Atomic Write** — Write to temp file, then rename to target
5. **Success Response** — Return `WriteOutput` with metadata

### Error/Success Matrix

| Condition | `is_error` | Content Type | Message |
|---|---|---|---|
| Parent directory doesn't exist | `true` | `Text` | "parent directory does not exist: ..." |
| Temp file write failed | `true` | `Text` | "failed to write file: ..." |
| Atomic rename failed | `true` | `Text` | "failed to finalize write: ..." |
| Success — new file | `false` | `Json` | `WriteOutput { created: true, ... }` |
| Success — overwrite | `false` | `Json` | `WriteOutput { created: false, ... }` |

## Usage

### As a Tool Definition

```rust
use operon_tools_fs_write::definition;

let def = definition();
// Send def to the model for tool registration
```

### Executing the Tool

```rust
use operon_tools_fs_write::execute;
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

let result = execute(
    ToolCallId("call_123".to_string()),
    json!({
        "path": "/path/to/file.txt",
        "content": "Hello, world!"
    })
).await.unwrap();

// result.is_error == false
// result.content contains WriteOutput with created=true, bytes_written=13
```

### In the Dispatcher

```rust
use operon_tools::dispatcher::Dispatcher;

let mut dispatcher = Dispatcher::new();
dispatcher.register_fs_tools();  // Registers write + other fs tools

// Later, when the model calls the write tool:
let result = dispatcher.dispatch(tool_call).await;
```

## Tool Definition

### Short Description (Normal Use)
Concise, states what the tool does and key constraints:
- Creates new files or overwrites existing ones
- Parent directory must exist
- Atomic writes (all-or-nothing)
- Complete replacement (no append/merge)

### Detailed Description (After Malformed Call)
Comprehensive explanation including:
- Input shapes and parameter descriptions
- Worked examples (create, overwrite)
- Parent directory requirement
- Atomic write guarantees
- Output field meanings
- When to use `write` vs `edit`
- Common mistakes and how to fix them
- All error messages and their causes

## When to Use Write vs Edit

### Use `write` for:
- Creating new files
- Complete file rewrites (most/all content changes)
- Replacing entire files with generated content

### Use `edit` for:
- Partial changes to existing files (one function, one import, a few lines)
- Precise, targeted modifications
- When you want to preserve most of the file

**Important**: Using `write` to make a small change requires sending the entire file content, which is inefficient. Use `edit` instead — it only needs the changed region.

## Testing

The crate includes 17 comprehensive tests covering:

**Success Cases:**
- Creating new files
- Overwriting existing files
- UTF-8 byte counting
- Atomic write cleanup
- Empty content
- Message format (Created vs Overwrote)

**Failure Cases:**
- Nonexistent parent directory
- File preservation on error

**Edge Cases:**
- Multiline content
- Large files (1MB)
- Special characters and emoji
- Files without trailing newlines
- Overwrite with shorter/longer content
- Sequential writes to same file

Run tests with:
```bash
cargo test -p operon-tools-fs-write
```

## Implementation Notes

### No Intermediate Directory Creation
The tool validates that the parent directory exists but does **not** create it. This is intentional — directory creation is a separate concern. If the parent doesn't exist, the tool returns an error and the file is not modified.

### Atomic Write Pattern
The implementation uses a proven atomic write pattern:
1. Create temp file in same directory as target (ensures same filesystem)
2. Write content to temp file
3. Atomically rename temp file to target path
4. On any failure, clean up temp file and return error

This guarantees that if the operation fails, the original file (if it existed) is completely untouched.

### No spawn_blocking
All operations are natively async (tokio::fs) or trivially fast in-memory (byte counting). No blocking operations.

### Error Handling
Every error path is explicit and returns a clear message. No panics in executor paths (except the intentional `unwrap_or_else` fallback for serialization bugs).

## Dependencies

All dependencies are workspace-managed:
- `tokio` — Async file I/O
- `serde` / `serde_json` — Serialization
- `thiserror` — Error types
- `operon-context-normalize-tools` — Tool result types
- `operon-tools-core` — Tiered tool definitions
- `tempfile` — Testing only

## Integration

The write tool is registered in the dispatcher alongside other filesystem tools (read, edit, grep, ls). It's available to the agent as soon as `dispatcher.register_fs_tools()` is called.

## Documentation

Every public item has comprehensive doc comments:
- Module-level `//!` comments explaining purpose and usage
- Function-level `///` comments with arguments, returns, and examples
- Inline comments in executor explaining each step

Doc tests are included and verified as part of the test suite.
