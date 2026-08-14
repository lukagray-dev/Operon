# operon-tools-fs-edit

The `edit` tool allows the Operon agent to make surgical, in-place modifications to existing files. It accepts an array of `old_string` -> `new_string` replacement pairs and supports **partial-success execution** with atomic disk writes.

## Features

- **Multi-hunk edits**: Supply one or more search-and-replace pairs per call.
- **Partial-success semantics**: When a multi-hunk call has some matching hunks and some failing hunks, all matching hunks are applied and atomically written to disk, while failed hunks are returned with structured error diagnostics for targeted retry.
- **Fuzzy sequence matching**: Incorporates a 6-pass fuzzy line seeker (`seek_sequence`) that tolerates trailing whitespace, indentation drift, Unicode quote/dash normalization, and casing differences when exact matching fails.
- **Ambiguity protection**: Returns an explicit ambiguity error if `old_string` matches multiple locations in the file.
- **Atomic I/O**: Successful edits are written to a temporary file in the target directory and atomically renamed onto the destination path.
- **Order-dependent execution**: Hunks run sequentially against the in-memory buffer, allowing subsequent hunks to build upon previous replacements.

## Schema

### Arguments (`EditArgs`)

```json
{
  "path": "/path/to/file.rs",
  "edits": [
    {
      "old_string": "fn old_func() {",
      "new_string": "fn new_func() {"
    }
  ]
}
```

- `path` (string, required): Absolute path to the file to edit. Also accepted as `file_path`.
- `edits` (array of `EditHunk`, min 1 item, required):
  - `old_string` (string, required): Target text to find and replace. Must match uniquely.
  - `new_string` (string, required): Replacement text. Must differ from `old_string`.

### Output (`EditOutput`)

```json
{
  "path": "/path/to/file.rs",
  "total_hunks": 2,
  "hunks_applied": 1,
  "hunks_failed": 1,
  "failures": [
    {
      "hunk_index": 1,
      "old_string": "missing_line",
      "reason": "old_string not found in file. The file may have changed since it was last read. Re-read the file and retry."
    }
  ],
  "message": "Partially applied: 1 of 2 edit(s) written to /path/to/file.rs; 1 edit(s) failed."
}
```

## Matching Strategy

1. **Exact substring match**: First checks byte-for-byte exact matches. If exactly 1 match is found, replaces it immediately.
2. **6-pass fuzzy sequence match**: If exact substring count is 0, falls back to line-based fuzzy seeking:
   - Pass 1: Exact line sequence match
   - Pass 2: Trailing whitespace ignored (rstrip)
   - Pass 3: Leading & trailing whitespace ignored (trim)
   - Pass 4: Unicode punctuation normalized (dashes, curly quotes, non-breaking spaces converted to ASCII)
   - Pass 5: Case-insensitive trimmed match
   - Pass 6: Case-insensitive Unicode normalized match

## Error Matrix

| Condition | `is_error` | Disk Modified | Result / Content |
|---|---|---|---|
| Malformed arguments / missing fields | `true` | No | Deserialization error |
| Empty `edits` array | `true` | No | Text error |
| `old_string == new_string` | `true` | No | Text error |
| File unreadable / missing | `true` | No | Text error |
| All hunks succeed ($M = \text{total}$) | `false` | Yes | `EditOutput` JSON |
| Partial success ($M > 0, N > 0$) | `true` | Yes ($M$ hunks) | `EditOutput` JSON with `failures` |
| All hunks fail ($M = 0$) | `true` | No | `EditOutput` JSON with `failures` |

## Testing

Run tests with:

```bash
cargo test -p operon-tools-fs-edit
```
