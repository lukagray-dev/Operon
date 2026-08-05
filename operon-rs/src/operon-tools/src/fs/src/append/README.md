# operon-tools-fs-append

The `append` tool for Operon's filesystem tool group. Appends text to the end of an existing file without modifying or reading existing content.

## Overview

`operon-tools-fs-append` implements a non-destructive append operation for the Operon agent. Unlike the `write` tool (which requires sending the entire file content) or the `edit` tool (which requires reading the file first), `append` only needs the new content to add.

### Key Characteristics

- **Non-destructive**: Existing file content is never modified or read
- **Atomic**: Uses OS-level append mode (`O_APPEND`) for atomic writes
- **Efficient**: No temp file, no read-before-write, minimal overhead
- **Safe**: File must exist; returns clear error if it doesn't
- **Async**: Built on `tokio::fs` for non-blocking I/O

## When to Use

Use `append` for:
- Adding entries to logs or accumulating output
- Appending to configuration files
- Building up file content incrementally
- Any operation where you're only adding to the end of an existing file

**Don't use `append` for:**
- Creating new files (use `write`)
- Modifying specific lines within a file (use `edit`)
- Replacing entire file content (use `write`)

## Architecture

The crate follows Operon's standard tool structure:

```
src/
├── lib.rs          # Public API: definition() and execute()
├── args.rs         # Input argument types (AppendArgs)
├── error.rs        # Error types (AppendToolError)
├── executor.rs     # Core append logic
├── output.rs       # Success output type (AppendOutput)
└── tests.rs        # Comprehensive test suite (19 tests)
```

### Module Responsibilities

- **`lib.rs`**: Exports public API and tool definition with tiered descriptions (short for normal use, detailed for error recovery)
- **`args.rs`**: Defines `AppendArgs` struct with `path` and `content` fields
- **`error.rs`**: Defines `AppendToolError` for argument parsing failures
- **`executor.rs`**: Implements the core append logic with full error handling
- **`output.rs`**: Defines `AppendOutput` struct returned on success
- **`tests.rs`**: 19 comprehensive tests covering success paths, failure paths, and edge cases

## API

### Public Functions

```rust
pub fn definition() -> TieredToolDefinition
```
Returns the tool definition with short and detailed descriptions for the model.

```rust
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, AppendToolError>
```
Executes the append tool. Returns `Ok(ToolResult)` with either success or failure (both as `Ok`, not `Err`). Returns `Err(AppendToolError::ArgsParse)` only if arguments are malformed.

### Input Schema

```json
{
  "path": "string (required) — absolute path to existing file",
  "content": "string (required) — text to append, must be non-empty"
}
```

### Output (Success)

```json
{
  "path": "string — echoed back for correlation",
  "bytes_appended": "number — UTF-8 byte count of appended content",
  "total_bytes": "number — total file size after append",
  "message": "string — human-readable summary"
}
```

### Error Cases

| Condition | Error Message |
|---|---|
| Content is empty | `"content is empty — nothing to append"` |
| File doesn't exist | `"file does not exist: {path}. Use the write tool to create new files."` |
| Path is a directory | `"path is a directory, not a file: {path}"` |
| File access fails | `"failed to access file: {path}: {error}"` |
| Open fails | `"failed to append to file: {error}"` |
| Write fails | `"failed to append to file: {error}"` |
| Flush fails | `"failed to flush file after append: {error}"` |

## Implementation Details

### Append Mode Strategy

The executor uses `tokio::fs::OpenOptions::new().append(true)` to open files in append mode. This:

1. Positions the write cursor at EOF at the OS level (atomically)
2. Ensures all writes go to the end of the file
3. Avoids race conditions without requiring a temp file
4. Is atomic per POSIX for writes under the pipe buffer size

**Why not temp file + rename?** That pattern is used by `write` because it needs to be atomic for complete file replacement. For append, OS-level append mode is simpler, faster, and equally safe.

### Error Handling Flow

1. **Empty content check** (fast fail) — reject immediately
2. **File existence check** — verify file exists and is not a directory
3. **Open with append mode** — get file handle
4. **Write content** — append bytes to file
5. **Flush** — ensure bytes are written
6. **Read metadata** (non-fatal) — get total file size for output

If any step fails, a `ToolResult` with `is_error: true` is returned. The file is never modified if an error occurs.

### Byte Counting

- `bytes_appended` is calculated as `args.content.len()` (UTF-8 byte count)
- `total_bytes` is read from file metadata after successful append
- If metadata read fails, `total_bytes` defaults to 0 (non-fatal — append succeeded)

## Testing

The crate includes 19 comprehensive tests:

### Success Path Tests (9)
- `test_basic_append` — append to file with existing content
- `test_multiple_appends` — sequential appends to same file
- `test_append_no_trailing_newline_warning` — content concatenation without newline
- `test_append_with_leading_newline` — proper line separation
- `test_bytes_appended_unicode` — UTF-8 byte counting (é = 2 bytes)
- `test_total_bytes_accurate` — total file size calculation
- `test_append_to_empty_file` — append to empty file
- `test_path_echoed_in_output` — path correlation
- `test_message_format` — human-readable output

### Failure Path Tests (4)
- `test_file_not_found` — error when file doesn't exist
- `test_path_is_directory` — error when path is directory
- `test_empty_content` — error when content is empty
- `test_existing_content_preserved_on_success` — file not modified on error

### Edge Case Tests (6)
- `test_multiline_append` — append multiple lines
- `test_large_append` — append 1 MB of content
- `test_special_characters_in_append` — Unicode and special chars
- `test_append_without_trailing_newline` — no automatic newline insertion
- `test_append_only_newline` — append just a newline
- `test_append_whitespace_only` — append whitespace

Run tests with:
```bash
cargo test -p operon-tools-fs-append
```

## Integration

The append tool is registered in the dispatcher alongside other filesystem tools:

```rust
// In operon-rs/src/operon-tools/src/dispatcher.rs
pub fn register_fs_tools(&mut self) {
    // ... other tools ...
    self.register(
        operon_tools_fs_append::definition(),
        |call_id, args| async move {
            operon_tools_fs_append::execute(call_id, args)
                .await
                .map_err(|e| e.to_string())
        },
    );
}
```

The tool is re-exported through the filesystem facade:

```rust
// In operon-rs/src/operon-tools/src/fs/src/lib.rs
pub use operon_tools_fs_append as append;
```

## Common Patterns

### Appending a log entry
```json
{
  "path": "/var/log/app.log",
  "content": "[2025-05-29 14:30:00] INFO: Operation completed\n"
}
```

### Adding to a configuration file
```json
{
  "path": "/etc/config.conf",
  "content": "\nnew_setting = true\n"
}
```

### Building up file content incrementally
```json
{
  "path": "/tmp/output.txt",
  "content": "Result: 42\n"
}
```

## Dependencies

All dependencies are workspace-managed:

- `tokio` — async runtime and filesystem operations
- `serde` / `serde_json` — serialization
- `thiserror` — error types
- `operon-context-normalize-tools` — tool types (ToolCallId, ToolResult, etc.)
- `operon-tools-core` — TieredToolDefinition

Dev dependencies:
- `tempfile` — temporary files for testing

## Design Principles

1. **Production-grade**: Robust error handling, comprehensive tests, clear error messages
2. **Non-destructive**: Existing content is never modified or read
3. **Efficient**: Minimal overhead, no unnecessary operations
4. **Clear semantics**: File must exist; content must be non-empty
5. **Well-documented**: Extensive inline comments, detailed docstrings, comprehensive README

## Future Considerations

- Append mode is currently synchronous at the OS level (atomic per POSIX)
- For very large appends (>pipe buffer size), consider chunking in future versions
- Symlink handling is implicit (follows symlinks via `tokio::fs::metadata`)
- No special handling for special files (devices, pipes, etc.) — returns OS error

## Related Tools

- **`write`** — Create new files or replace entire file content
- **`edit`** — Modify specific lines within a file
- **`read`** — Read one or multiple files
- **`grep`** — Search for patterns in files
- **`ls`** — List directory contents
