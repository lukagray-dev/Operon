# operon-tools-fs-delete

The `delete` tool for Operon's filesystem tool group. Safely deletes files or directories with two modes: trash (recoverable) or permanent (irreversible).

## Overview

This crate implements a production-grade file and directory deletion tool for the Operon agent. It provides:

- **Trash mode (default)**: Moves files/directories to system trash (macOS Trash, Windows Recycle Bin, Linux trash-spec). Recoverable by the user.
- **Permanent mode**: Permanently deletes files/directories using `remove_file` / `remove_dir_all`. Irreversible.
- **Safe defaults**: Defaults to trash mode to prevent accidental data loss.
- **Full support**: Handles individual files, entire directory trees, and symlinks.
- **Async-safe**: All blocking operations run in `tokio::task::spawn_blocking` to avoid blocking the async runtime.

## Architecture

### Module Structure

```
src/
├── lib.rs          # Public API, tool definition (short + detailed tiers)
├── args.rs         # Argument deserialization (DeleteArgs)
├── output.rs       # Success output types (DeleteOutput, DeletedKind)
├── executor.rs     # Core deletion logic
├── error.rs        # Error types (DeleteToolError)
└── tests.rs        # Comprehensive test suite (18 tests)
```

### Key Design Decisions

1. **Tiered Descriptions**: The tool provides two descriptions:
   - `short`: Concise, sent to the model under normal conditions
   - `detailed`: Comprehensive, sent after a malformed call to help the model recover

2. **Blocking Operations**: The `trash` crate and `std::fs` operations are synchronous. They run in `spawn_blocking` to prevent blocking the async runtime.

3. **Error Handling**: All errors are returned as `ToolResult { is_error: true, content: ToolContent::Text(...) }`. Only argument parsing errors propagate as `Err(DeleteToolError)`.

4. **Safety by Default**: `permanent` defaults to `false` (trash mode). The model must explicitly opt into permanent deletion.

## Usage

### As a Tool Consumer

```rust
use operon_tools_fs_delete::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    // Get the tool definition for the model
    let def = definition();
    println!("Tool: {}", def.short.name);

    // Execute the tool
    let result = execute(
        ToolCallId("call_123".to_string()),
        json!({
            "path": "/tmp/file.txt",
            "permanent": false
        })
    ).await.unwrap();

    println!("Success: {}", !result.is_error);
}
```

### In the Dispatcher

The delete tool is registered in `operon-rs/src/operon-tools/src/dispatcher.rs`:

```rust
dispatcher.register(
    operon_tools_fs_delete::definition(),
    |call_id, args| async move {
        operon_tools_fs_delete::execute(call_id, args)
            .await
            .map_err(|e| e.to_string())
    },
);
```

## API

### `definition() -> ToolDefinition`

Returns the tool's canonical definition with industry-standard JSON Schema parameter specifications.
- Full parameter documentation
- Deletion modes explained in detail
- Worked examples (trash file, permanent delete, directory deletion)
- Common mistakes and how to avoid them
- Error messages and recovery guidance
- Strong safety warnings about permanent deletion

### `execute(call_id: ToolCallId, args_json: serde_json::Value) -> Result<ToolResult, DeleteToolError>`

Executes the delete tool with the given arguments.

**Arguments:**
- `call_id`: Unique identifier for this tool call
- `args_json`: Raw JSON arguments from the model

**Returns:**
- `Ok(ToolResult)` with either success (JSON DeleteOutput) or failure (Text error message)
- `Err(DeleteToolError::ArgsParse)` if arguments are malformed

**Success output** (DeleteOutput):
```json
{
  "path": "/path/to/file.txt",
  "kind": "file",
  "permanent": false,
  "message": "Moved /path/to/file.txt to trash (file)"
}
```

**Failure output** (Text):
```
"path does not exist: /path/to/file.txt"
```

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Absolute path to the file or directory to delete."
    },
    "permanent": {
      "type": "boolean",
      "default": false,
      "description": "If false (default), move to system trash (recoverable). If true, permanently delete. Prefer false."
    }
  },
  "required": ["path"]
}
```

## Deletion Modes

### Trash Mode (permanent: false, default)

Moves the target to the system trash. The file is recoverable by the user.

- **macOS**: Moved to `~/Trash`
- **Windows**: Moved to Recycle Bin
- **Linux**: Moved to `~/.local/share/Trash` (trash-spec)

**Use for:**
- Almost all deletions
- When the user might want to recover the file
- When you're not 100% sure the path is correct

### Permanent Mode (permanent: true)

Permanently deletes the target using `remove_file` (files) or `remove_dir_all` (directories). Irreversible.

**Use only for:**
- Temp files that must not persist in trash
- Secrets or sensitive data that must be removed from disk
- Cleaning up after a build or test run

**WARNING**: Permanent deletion is irreversible. If the wrong path is deleted, the data is unrecoverable.

## Error Handling

| Condition | Error Message |
|---|---|
| Path does not exist | `"path does not exist: {path}"` |
| Failed to access path | `"failed to access path: {path}: {error}"` |
| Trash operation failed | `"failed to move to trash: {error}"` |
| Permanent file delete failed | `"failed to delete file: {error}"` |
| Permanent dir delete failed | `"failed to delete directory: {error}"` |
| spawn_blocking panicked | `"internal error: delete task panicked"` |

## Test Coverage

The crate includes 18 comprehensive tests covering:

### Success Paths
- `test_trash_file`: Delete a file to trash
- `test_trash_directory`: Delete a directory to trash
- `test_permanent_delete_file`: Permanently delete a file
- `test_permanent_delete_directory`: Permanently delete a directory with nested contents
- `test_default_permanent_is_false`: Verify default behavior
- `test_path_echoed_in_output`: Verify output correlation
- `test_kind_file_vs_dir`: Verify kind detection
- `test_message_format_trash`: Verify trash message format
- `test_message_format_permanent`: Verify permanent message format

### Failure Paths
- `test_path_not_found`: Handle nonexistent path
- `test_nonexistent_nested_path`: Handle nested nonexistent path

### Edge Cases
- `test_delete_nested_directory_structure`: Complex nested directories
- `test_delete_empty_directory`: Empty directory deletion
- `test_delete_file_with_special_characters_in_name`: Special characters in filename
- `test_delete_large_directory`: 100+ files in directory
- `test_delete_file_with_unicode_name`: Unicode filenames
- `test_delete_file_with_unicode_content`: Unicode file content
- `test_permanent_flag_true_vs_false`: Both modes in same test

**All tests pass**: ✓ 18 unit tests + 2 doc tests

## Dependencies

- `tokio`: Async runtime with `spawn_blocking` support
- `trash`: Cross-platform trash/recycle bin support (v5)
- `serde` / `serde_json`: Serialization/deserialization
- `thiserror`: Error type derivation
- `operon-context-normalize-tools`: Tool types (ToolCallId, ToolResult, etc.)
- `operon-tools-core`: ToolProgress and error types

## Integration

### Workspace Registration

The delete tool is registered in the workspace root `Cargo.toml`:

```toml
[workspace.members]
"operon-rs/src/operon-tools/src/fs/src/delete"

[workspace.dependencies]
trash = "5"
operon-tools-fs-delete = { path = "operon-rs/src/operon-tools/src/fs/src/delete" }
```

### Module Re-exports

The tool is re-exported through the fs module hierarchy:

- `operon-tools-fs/Cargo.toml`: Depends on `operon-tools-fs-delete`
- `operon-tools-fs/src/lib.rs`: Re-exports as `pub use operon_tools_fs_delete as delete`
- `operon-tools/Cargo.toml`: Depends on `operon-tools-fs-delete`
- `operon-tools/src/dispatcher.rs`: Registers in `register_fs_tools()`

## Code Quality

- **No warnings**: Zero compiler warnings
- **No unwrap() in hot paths**: Only in `serde_json::to_value` fallback with `unwrap_or_else`
- **Complete documentation**: Every public item has doc comments
- **Module-level docs**: `//!` on every file explaining purpose
- **Inline comments**: Detailed explanations of complex logic (spawn_blocking, error handling)
- **Production-grade**: Follows industrial architecture standards

## Safety Considerations

1. **Path Validation**: The tool validates that the path exists before deletion
2. **Kind Detection**: Distinguishes between files and directories to use appropriate deletion method
3. **Symlink Handling**: Deletes the symlink itself, not the target
4. **Async Safety**: All blocking operations run in `spawn_blocking`
5. **Error Recovery**: Comprehensive error messages help the model recover from mistakes
6. **Safe Defaults**: Trash mode by default prevents accidental permanent deletion

## Performance

- **Trash mode**: O(1) move operation (OS-level)
- **Permanent file delete**: O(1) remove operation
- **Permanent directory delete**: O(n) where n = total files/subdirs in tree
- **No temp files**: Trash mode uses OS trash, permanent mode uses direct removal
- **Async-safe**: Blocking operations don't block the async runtime

## Future Enhancements

Potential improvements (not currently implemented):

- Batch deletion (multiple paths in one call)
- Dry-run mode (preview what would be deleted)
- Recursive pattern matching (delete all matching paths)
- Confirmation prompts (for safety-critical deletions)
- Deletion history/audit log

## License

AGPL-3.0 (same as Operon)
