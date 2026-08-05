# operon-tools-fs-edit

Exact-string file editing tool for the Operon agent with atomic writes and multi-hunk support.

## Overview

The `edit` tool allows the Operon agent to modify files by replacing exact text strings. It is designed for precision and safety:

- **Exact-string matching**: Each replacement must match exactly once in the file. Zero or multiple matches are errors, preventing silent partial edits.
- **Multi-hunk edits**: Apply one or more replacements in a single call, all committed atomically.
- **Atomic writes**: Either all hunks succeed and the file is written, or none succeed and the file remains unchanged.
- **In-order application**: Hunks are applied sequentially on the in-memory content, so later hunks can depend on earlier replacements.

## Usage

### Basic Single Edit

Replace a single occurrence of text:

```json
{
  "path": "/path/to/file.rs",
  "edits": [
    {
      "old_string": "fn old_name() {",
      "new_string": "fn new_name() {"
    }
  ]
}
```

### Multi-Hunk Edit

Apply multiple replacements in one call:

```json
{
  "path": "/path/to/file.rs",
  "edits": [
    {
      "old_string": "import { oldFunc } from './lib';",
      "new_string": "import { newFunc } from './lib';"
    },
    {
      "old_string": "oldFunc(x, y)",
      "new_string": "newFunc(x, y)"
    },
    {
      "old_string": "// TODO: refactor oldFunc",
      "new_string": "// TODO: refactor newFunc"
    }
  ]
}
```

All three edits are applied in order and committed atomically.

## Key Concepts

### Exact-String Matching

Each `old_string` must match **exactly once** in the file:

- **Zero matches**: The file changed since it was last read. Re-read the file and retry.
- **Multiple matches**: The `old_string` is ambiguous. Include more surrounding context (function signature, preceding comment, more unique lines) to make it unique.

This constraint ensures determinism and prevents silent partial edits.

### Hunk Application Order

Hunks are applied sequentially on the in-memory content. Later hunks see the post-edit state from earlier hunks.

**Example**: If hunk 0 replaces "foo" with "bar", and hunk 1 searches for "bar", hunk 1 will find the result of hunk 0.

This allows hunks to touch overlapping or dependent regions, as long as the `old_string` for each hunk matches the state after all previous hunks have been applied.

### Atomic Writes

All-or-nothing semantics: if any hunk fails, the file is **not modified at all**. This prevents partial edits on disk.

The tool uses a temporary file in the same directory as the target, then atomically renames it to ensure the write is atomic even on filesystems that don't support atomic operations natively.

### Whitespace Exactness

Tabs vs spaces, trailing newlines, indentation — all must match exactly as they appear in the file.

**Important**: The line number prefix from the `read` tool output (e.g., `"  123 | "`) is display-only and must **not** be included in `old_string`.

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `old_string not found in file` | File changed since last read | Re-read the file with `read` tool, then retry |
| `old_string matched K times — ambiguous` | Multiple matches found | Add more surrounding context to make `old_string` unique |
| `old_string and new_string are identical` | No change would be made | Ensure `new_string` differs from `old_string` |
| `edits array must contain at least one hunk` | Empty edits array | Provide at least one edit |
| `failed to read file` | File doesn't exist or permission denied | Verify the path and permissions |
| `failed to write temp file` | Disk full or permission denied | Check disk space and permissions |
| `failed to rename temp file to target` | Atomic rename failed | Check permissions and disk space |

## Common Mistakes

### Mistake #1: old_string is too short and matches multiple places

**Wrong**:
```json
{
  "old_string": "}",
  "new_string": "} // end of function"
}
```

The closing brace `}` appears many times in the file, causing an ambiguity error.

**Right**:
```json
{
  "old_string": "fn process_data() {\n    // implementation\n}",
  "new_string": "fn process_data() {\n    // implementation\n} // end of process_data"
}
```

Include enough surrounding context to make the match unique.

### Mistake #2: Not re-reading after an external edit

If the file changed on disk (e.g., another tool or editor modified it), your `old_string` may no longer match.

**Solution**: Always re-read the file with the `read` tool before retrying the edit.

### Mistake #3: Including the line number prefix from read output

The `read` tool output shows:
```
  123 | fn foo() {
```

The `"  123 | "` prefix is display-only. Your `old_string` should be just:
```
fn foo() {
```

### Mistake #4: Forgetting to include newlines in multiline edits

If you're replacing a multiline block, include the newlines:

**Wrong**:
```json
{
  "old_string": "fn old() {\n    println!(\"hello\");\n}",
  "new_string": "fn new() {\n    println!(\"goodbye\");\n}"
}
```

**Right** (if the file has a trailing newline after the closing brace):
```json
{
  "old_string": "fn old() {\n    println!(\"hello\");\n}\n",
  "new_string": "fn new() {\n    println!(\"goodbye\");\n}\n"
}
```

## Implementation Details

### Architecture

The crate follows the standard Operon tool structure:

- **`lib.rs`**: Public API (`definition()`, `execute()`)
- **`args.rs`**: Argument deserialization (EditArgs, EditHunk)
- **`error.rs`**: Error types (EditToolError)
- **`output.rs`**: Success output (EditOutput)
- **`executor.rs`**: Core logic (file I/O, hunk application, atomic writes)
- **`tests.rs`**: Comprehensive test suite

### Execution Flow

1. **Validate arguments** (fast fail, before reading file):
   - Check that `edits` array is non-empty
   - Check that no hunk has `old_string == new_string`

2. **Read the file**:
   - Return error if file doesn't exist or is unreadable

3. **Apply hunks in order**:
   - For each hunk, count occurrences of `old_string`
   - If count != 1, return error immediately
   - Otherwise, apply the replacement to the in-memory content

4. **Atomic write** (only if all hunks succeeded):
   - Write to a temporary file in the same directory
   - Atomically rename temp file to target path
   - Clean up temp file on failure

5. **Return success** with EditOutput containing:
   - `path`: The file that was edited
   - `hunks_applied`: Number of hunks applied
   - `message`: Human-readable summary

### Error/Success Matrix

| Condition | `is_error` | Content |
|-----------|-----------|---------|
| Args failed to parse | `true` | `Text(reason)` — handled by dispatcher |
| File not found / unreadable | `true` | `Text("failed to read file: ...")` |
| `edits` array is empty | `true` | `Text("edits array must contain at least one hunk")` |
| `old_string == new_string` on hunk N | `true` | `Text("hunk N: old_string and new_string are identical...")` |
| `old_string` not found (hunk N) | `true` | `Text("hunk N: old_string not found in file...")` |
| `old_string` matches K>1 times (hunk N) | `true` | `Text("hunk N: old_string matched K times — ambiguous...")` |
| Temp file write failed | `true` | `Text("failed to write temp file: ...")` |
| Atomic rename failed | `true` | `Text("failed to rename temp file to target: ...")` |
| All hunks applied, file written | `false` | `Json(EditOutput { ... })` |

All `is_error: true` results use `ToolContent::Text` for easy model parsing.

## Testing

The crate includes 15 comprehensive tests covering:

- **Success paths**: Single edit, multi-hunk edit, atomic write, file_path alias
- **Failure paths**: Zero match, multiple matches, identical strings, partial failure, nonexistent file, empty edits
- **Edge cases**: Hunk order dependency, multiline edits, whitespace exactness, empty files, files without trailing newlines

Run tests with:
```bash
cargo test -p operon-tools-fs-edit
```

All tests pass with zero warnings.

## Integration

The `edit` tool is registered in the Operon dispatcher via `register_fs_tools()`:

```rust
self.register(
    operon_tools_fs_edit::definition(),
    |call_id, args| async move {
        operon_tools_fs_edit::execute(call_id, args)
            .await
            .map_err(|e| e.to_string())
    },
);
```

It is available to the model as part of the filesystem tool group alongside `read`, `grep`, and `ls`.

## Dependencies

- `tokio`: Async file I/O
- `serde` / `serde_json`: Argument deserialization and output serialization
- `thiserror`: Error type derivation
- `operon-context-normalize-tools`: Tool infrastructure (ToolCallId, ToolResult, etc.)
- `operon-tools-core`: Tiered tool definitions

All dependencies are already in the workspace.

## Performance

- **File I/O**: Fully async via tokio
- **String operations**: In-memory, no external processes
- **Atomic writes**: Single temp file + rename operation
- **No blocking**: All operations are natively async

## Future Enhancements

Potential improvements (not currently implemented):

- Regex-based matching (currently exact-string only)
- Batch file edits in a single call
- Diff preview before applying edits
- Undo/rollback support
- Backup creation before edits

## License

AGPL-3.0 — See the workspace root for details.
