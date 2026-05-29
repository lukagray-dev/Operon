# operon-tools-fs-ls

The `ls` tool for the Operon agent's filesystem group. Provides single-level directory listing with entry type classification, metadata collection, and glob-pattern exclusion.

## Overview

`operon-tools-fs-ls` implements a directory listing tool that returns structured information about files and directories at a given path. Unlike recursive tools like `grep`, `ls` lists only the immediate contents of a directory (single level, not recursive).

## Features

- **Single-level directory listing** — lists immediate contents only, no recursion
- **Entry type classification** — distinguishes between files, directories, and symlinks
- **Metadata collection** — captures file size (bytes) and last-modified timestamp (Unix seconds)
- **Glob-pattern exclusion** — filter entries by name using glob patterns (e.g., `*.lock`, `node_modules`)
- **Truncation handling** — caps results at 1000 entries to prevent overwhelming the model
- **Robust error handling** — distinguishes between different failure modes (not found, permission denied, file vs directory)
- **Hidden files included** — entries starting with `.` are included by default (unlike some shell `ls` implementations)

## Architecture

The crate follows the same 5-file structure as other Operon tools (`read`, `grep`):

```
src/
├── lib.rs          — Public API: definition() and execute()
├── args.rs         — Input argument types (LsArgs)
├── output.rs       — Output types (LsOutput, LsEntry, EntryKind)
├── error.rs        — Error types (LsToolError)
├── executor.rs     — Core listing logic
└── tests.rs        — Comprehensive test suite
```

### Module Responsibilities

- **lib.rs** — Exports the tool definition (short + detailed descriptions) and the async execute function
- **args.rs** — Deserializes model input into `LsArgs` (path + optional ignore patterns)
- **output.rs** — Defines the structured result format returned to the model
- **error.rs** — Defines top-level argument parsing errors
- **executor.rs** — Implements the core directory listing logic (glob building, entry collection, sorting, truncation)
- **tests.rs** — Unit tests covering basic listing, exclusion, errors, hidden files, truncation, and metadata

## Usage

### As a Tool Consumer

```rust
use operon_tools_fs_ls::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

// Get the tool definition to register with the model
let def = definition();

// Execute the tool
let args = json!({
    "path": "/home/user/project",
    "ignore": ["*.lock", "node_modules", ".git"]
});

let result = execute(
    ToolCallId("call_123".to_string()),
    args
).await.unwrap();
```

### As a Model Tool

The model calls the tool with JSON arguments:

```json
{
  "path": "/home/user/project",
  "ignore": ["*.lock", "node_modules", ".git", "target"]
}
```

The tool returns a JSON response:

```json
{
  "path": "/home/user/project",
  "entry_count": 5,
  "truncated": false,
  "entries": [
    {
      "name": "src",
      "kind": "DIR",
      "size_bytes": null,
      "modified_unix": 1704067200
    },
    {
      "name": "Cargo.toml",
      "kind": "FILE",
      "size_bytes": 1024,
      "modified_unix": 1704067200
    },
    {
      "name": "README.md",
      "kind": "FILE",
      "size_bytes": 2048,
      "modified_unix": 1704067200
    }
  ],
  "error": null
}
```

## Input Parameters

### `path` (required, string)

Absolute path to the directory to list.

- Must be a directory. Passing a file path returns an error result (not an Err).
- Must be an absolute path (relative paths are not supported).

### `ignore` (optional, array of strings)

Glob patterns to exclude entries by name.

- Patterns are matched against the **entry name only**, not the full path.
- Examples: `["*.lock", "node_modules", ".git", "target", ".*"]`
- If any pattern fails to compile, the entire listing fails with an error.
- Default: empty (no exclusions).

## Output Format

### Top-level fields

- **path** — The directory that was listed (echoed back for correlation)
- **entry_count** — Total number of entries in the listing (after exclusions)
- **truncated** — Boolean. True if more than 1000 entries exist (results are capped)
- **entries** — Array of directory entries, sorted: directories first (alphabetical), then files/symlinks (alphabetical)
- **error** — Human-readable error if the path could not be listed. When populated, entries is empty

### Entry fields

Each entry in `entries` contains:

- **name** — Entry name (not full path)
- **kind** — Entry type: one of `FILE`, `DIR`, `SYMLINK`
- **size_bytes** — File size in bytes (only for files; `null` for dirs/symlinks)
- **modified_unix** — Last modified timestamp as Unix seconds (`null` if unavailable)

## Behavior

### Single-level only

Does not recurse into subdirectories. Use `grep` or `read` for recursive operations.

### Hidden files included

Entries starting with `.` (e.g., `.git`, `.env`) ARE included by default. Use `ignore: [".*"]` to exclude them.

### Symlinks

Followed for metadata (size/modified time of the target). If the target is missing, metadata is `null` but the symlink is still listed as `SYMLINK`.

### Metadata failures

If metadata cannot be retrieved for an entry, the entry is still included but `size_bytes` and `modified_unix` are `null`.

### Truncation

If a directory contains more than 1000 entries, results are capped at 1000 and `truncated` is set to `true`. Call the tool multiple times with different ignore patterns if needed.

### Sorting

Directories come first (alphabetical, case-insensitive), then files and symlinks (alphabetical, case-insensitive).

## Error Handling

The tool distinguishes between different failure modes:

- **Path not found** — `error: "path not found: /path/to/dir"`
- **Permission denied** — `error: "permission denied: /path/to/dir"`
- **File instead of directory** — `error: "path is a file, not a directory: /path/to/file"`
- **Invalid glob pattern** — `error: "invalid ignore pattern '*.{': ..."`

All errors are returned as `LsOutput` with `error` populated and `entries` empty. The tool never returns `Err` for these cases — they are always `Ok(ToolResult)` with error details in the JSON.

## Common Mistakes

- **Passing a file path instead of a directory** — Returns an error result (not Err)
- **Using relative paths** — May fail or list unexpected directory
- **Expecting recursive output** — Use `grep` or `read` for recursive operations
- **Using ignore patterns that match full paths** — Patterns match entry names only. Use `ignore: ["node_modules"]` not `ignore: ["**/node_modules"]`
- **Forgetting to exclude hidden files** — Use `ignore: [".*"]` if needed

## Testing

The crate includes comprehensive tests:

```bash
cargo test -p operon-tools-fs-ls
```

Tests cover:
- Basic listing with mixed file/directory entries
- Glob pattern exclusion
- File path error handling
- Nonexistent path error handling
- Hidden file inclusion
- Truncation at 1000 entries
- Metadata collection (size, modified time)
- Invalid glob pattern handling
- Case-insensitive sorting

## Integration

The tool is registered in the dispatcher via `operon-tools`:

```rust
pub fn register_fs_tools(&mut self) {
    // ... other tools ...
    self.register(
        operon_tools_fs_ls::definition(),
        |call_id, args| async move {
            operon_tools_fs_ls::execute(call_id, args)
                .await
                .map_err(|e| e.to_string())
        },
    );
}
```

## Dependencies

- **tokio** — Async file I/O
- **globset** — Glob pattern matching
- **serde/serde_json** — Serialization
- **thiserror** — Error types
- **operon-context-normalize-tools** — Tool infrastructure
- **operon-tools-core** — Shared tool types

## Performance

- **Async I/O** — Uses `tokio::fs` for non-blocking directory operations
- **Streaming** — Entries are collected as they are read, not buffered entirely
- **Truncation** — Stops reading after 1000 entries to prevent excessive I/O
- **Glob compilation** — Patterns are compiled once at the start, not per-entry

## Limitations

- **Single-level only** — Does not recurse into subdirectories
- **1000 entry cap** — Results are truncated if a directory contains more entries
- **Name-only matching** — Glob patterns match entry names, not full paths
- **No sorting options** — Always sorts directories first, then files (case-insensitive)
- **No filtering by type** — Cannot filter to only files or only directories

## License

AGPL-3.0 — See the workspace root for details.
