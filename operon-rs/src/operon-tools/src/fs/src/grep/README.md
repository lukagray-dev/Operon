# operon-tools-fs-grep

The `grep` tool for the Operon agent's filesystem group. Searches files and directories for regex patterns with support for recursive directory walking, filename filtering, context lines, and per-file match reporting.

## Features

- **Regex pattern matching**: Full Rust regex syntax support with case-sensitive/insensitive modes
- **Recursive directory walking**: Automatically walks directories with gitignore rules respected
- **Filename glob filtering**: Use patterns like `*.rs` or `*.{ts,tsx}` to filter files during directory walks
- **Context lines**: Include N lines before/after each match for better context
- **Per-file match reporting**: Each file gets its own result with line numbers and match counts
- **Match limits**: Caps results at 300 matches to prevent context overflow
- **Size limits**: Skips files larger than 10 MB with clear error messages
- **Binary file detection**: Automatically detects and skips binary files
- **Concurrent file processing**: Searches multiple files efficiently

## Architecture

This crate is a **pure tool implementation** with zero dependencies on the Operon context pipeline. It only depends on:

- `operon-context-normalize-tools` — for canonical tool types (`ToolDefinition`, `ToolResult`, etc.)
- `grep-searcher` and `grep-regex` — for high-performance regex searching
- `ignore` — for gitignore-aware directory walking
- Standard async runtime (`tokio`) and serialization (`serde`, `serde_json`)

### File Structure

```
src/
├── lib.rs        ← Public API, re-exports, tiered tool definition
├── args.rs       ← GrepArgs deserialization with pattern, paths, filters
├── output.rs     ← GrepOutput, FileGrepResult, GrepLine (result types)
├── error.rs      ← GrepToolError (thiserror)
├── executor.rs   ← Core search logic using grep-searcher + ignore crates
└── tests.rs      ← Comprehensive test suite
```

## Usage

### Basic Example

```rust
use operon_tools_fs_grep::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    // 1. Get the tiered tool definition
    let def = definition();
    println!("Tool name: {}", def.name());
    println!("Short description: {}", def.short.description);
    
    // 2. Search for a pattern in files
    let args = json!({
        "pattern": "fn main",
        "paths": ["src/"]
    });
    
    let result = execute(
        ToolCallId("call_123".to_string()),
        args
    ).await.unwrap();
    
    // 3. The result contains per-file matches with line numbers
    println!("{:?}", result);
}
```

### Search with Filename Filter

```rust
// Search only Rust files for "TODO" comments
let args = json!({
    "pattern": "TODO",
    "paths": ["src/", "tests/"],
    "include": "*.rs"
});

let result = execute(call_id, args).await.unwrap();
```

### Case-Insensitive Search with Context

```rust
// Search for "error" (case-insensitive) with 2 lines of context
let args = json!({
    "pattern": "error",
    "paths": ["logs/"],
    "case_insensitive": true,
    "context_lines": 2
});

let result = execute(call_id, args).await.unwrap();
```

### Search Multiple File Types

```rust
// Search TypeScript and TSX files
let args = json!({
    "pattern": "useState",
    "paths": ["src/components/"],
    "include": "*.{ts,tsx}"
});
```

## Tool Definition

The tool exposes a **tiered definition** with short and detailed descriptions:

### Short Description (Normal Use)

```
Searches files and directories for a regex pattern.
Pass `pattern` (regex string) and `paths` (array of file/dir paths).
Directories are walked recursively respecting .gitignore.
Use `include` to filter by filename glob (e.g., "*.rs").
Results capped at 300 matches. Files >10 MB are skipped.
```

### Detailed Description (After Malformed Call)

Full explanation with:
- Input shapes and parameter descriptions
- Regex syntax quick reference
- Response format with examples
- Behavior details (directory walking, pattern matching, limits)
- Common mistakes (forgetting to escape regex special characters, etc.)

The dispatcher automatically switches to the detailed description when the model sends malformed arguments, helping it recover gracefully.

## Tool Definition Schema

```json
{
  "name": "grep",
  "description": "Searches files and directories for a regex pattern...",
  "parameters": {
    "type": "object",
    "properties": {
      "pattern": {
        "type": "string",
        "description": "Regex pattern to search for. Uses Rust regex syntax."
      },
      "paths": {
        "type": "array",
        "items": { "type": "string" },
        "minItems": 1,
        "description": "Files or directories to search. Directories are walked recursively."
      },
      "include": {
        "type": "string",
        "description": "Optional glob pattern to filter files by name (e.g., \"*.rs\")."
      },
      "case_insensitive": {
        "type": "boolean",
        "description": "Case-insensitive matching. Default: false."
      },
      "context_lines": {
        "type": "integer",
        "minimum": 0,
        "description": "Number of context lines before/after each match. Default: 0."
      }
    },
    "required": ["pattern", "paths"]
  }
}
```

## Output Format

The tool returns a `ToolResult` with `is_error: false` for successful searches (even if no matches found). Invalid regex patterns return `is_error: true`. Per-file errors are embedded in the JSON content:

```json
{
  "total_matches": 5,
  "files_with_matches": 2,
  "truncated": false,
  "files": [
    {
      "path": "src/main.rs",
      "match_count": 3,
      "matches": [
        {
          "line_no": 10,
          "content": "fn main() {",
          "is_match": true
        },
        {
          "line_no": 11,
          "content": "    println!(\"Hello\");",
          "is_match": false
        }
      ]
    },
    {
      "path": "src/lib.rs",
      "match_count": 2,
      "matches": [
        {
          "line_no": 42,
          "content": "pub fn main_function() {",
          "is_match": true
        }
      ]
    }
  ]
}
```

### Output Fields

- **`total_matches`**: Total number of matching lines across all files (excludes context lines)
- **`files_with_matches`**: Number of files that had at least one match
- **`truncated`**: `true` if the 300 match limit was hit (results are incomplete)
- **`files`**: Array of per-file results (only includes files with matches or errors)

### Per-File Result

- **`path`**: The file that was searched
- **`match_count`**: Number of matching lines in this file
- **`matches`**: Array of matching lines and context lines
  - **`line_no`**: 1-indexed line number in the source file
  - **`content`**: Line text with trailing newline removed
  - **`is_match`**: `true` for matching lines, `false` for context lines
- **`error`**: Present only if the file could not be searched (permission denied, binary file, too large, etc.)

## Error Handling

### Top-Level Errors

Returns `Err(GrepToolError::ArgsParse)` only if the JSON arguments are malformed (e.g., missing `pattern` or `paths` field).

Returns `is_error: true` in the ToolResult for:
- **Invalid regex pattern**: `"Invalid regex pattern: ..."`
- **Internal serialization bugs**: `"Internal error: failed to serialize grep output: ..."`

### Per-File Errors

Individual file failures are captured in `FileGrepResult`:

- **File too large**: `"File too large, skipped (>10 MB): X bytes"`
- **Binary file**: `"Binary file, skipped"`
- **Permission denied**: `"Failed to access file: Permission denied"`
- **Search failed**: `"Search failed: ..."`

## Limits and Constraints

### Match Limit

- **Maximum 300 total matches** across all files
- When the limit is hit, `truncated: true` is set in the response
- The current file being searched is finished, but subsequent files are skipped
- Refine your search with more specific patterns or narrower paths if truncated

### File Size Limit

- **Maximum 10 MB per file**
- Files larger than this are skipped with an error message
- This prevents memory issues and excessive processing time

### Binary Files

Files containing null bytes (`\0`) are automatically detected and skipped. This prevents attempting to search images, executables, or other non-text files.

## Directory Walking

- Directories in `paths` are walked recursively
- **Gitignore rules are respected** — files in `.gitignore` are automatically skipped
- Hidden files and directories (starting with `.`) are skipped by default
- The `include` glob filter is applied during the walk
- **Note**: `include` only affects directory walks, not files listed directly in `paths`

## Regex Syntax

The tool uses Rust regex syntax. Common patterns:

- `.` — any character (except newline)
- `*` — zero or more of the preceding
- `+` — one or more of the preceding
- `?` — zero or one of the preceding
- `^` — start of line
- `$` — end of line
- `\b` — word boundary
- `[abc]` — any of a, b, or c
- `[^abc]` — any character except a, b, or c
- `(a|b)` — either a or b
- `\d` — digit (0-9)
- `\w` — word character (a-z, A-Z, 0-9, _)
- `\s` — whitespace

**Escape special characters** with `\` for literal matching: `\.`, `\*`, `\(`, `\)`, `\[`, `\]`, etc.

Full syntax reference: https://docs.rs/regex/latest/regex/#syntax

## Common Mistakes

1. **Forgetting to escape regex special characters**:
   - Wrong: `{"pattern": "main()", "paths": ["src/"]}`  ← `()` are regex groups
   - Right: `{"pattern": "main\\(\\)", "paths": ["src/"]}`  ← escaped for literal match

2. **Expecting `include` to filter direct file paths**:
   - `include` only affects directory walks, not files listed directly in `paths`
   - If `paths: ["main.py", "test.js"]` and `include: "*.py"`, both files are still searched

3. **Passing `paths` as a string instead of an array**:
   - Wrong: `{"pattern": "TODO", "paths": "src/"}`
   - Right: `{"pattern": "TODO", "paths": ["src/"]}`

4. **Assuming `is_match: false` means no match**:
   - `is_match: false` indicates a context line, not a failed match
   - Context lines are included via the `context_lines` parameter

5. **Not checking the `truncated` field**:
   - If `truncated: true`, results are incomplete
   - Refine the search (more specific pattern, narrower paths, use `include` filter)

## Testing

Run the comprehensive test suite:

```bash
cargo test -p operon-tools-fs-grep
```

Run the example:

```bash
cargo run --example basic_usage -p operon-tools-fs-grep
```

## Design Constraints

- **No `unwrap()` or `expect()`** in production paths — all errors are handled explicitly
- **No `unsafe` code**
- **Zero context pipeline dependencies** — only depends on `operon-context-normalize-tools`
- **All public types derive `Debug`**
- **Serializable types derive `Serialize`/`Deserialize`**
- **Comprehensive inline documentation** on every type, field, and function

## Performance

- **Concurrent file processing**: Multiple files are searched in parallel (bounded by semaphore)
- **Efficient regex engine**: Uses the `grep-regex` crate (same engine as ripgrep)
- **Gitignore-aware walking**: Uses the `ignore` crate for fast directory traversal
- **Blocking task isolation**: CPU-bound search work runs in `tokio::task::spawn_blocking`

## License

AGPL-3.0 — See the workspace root for details.
