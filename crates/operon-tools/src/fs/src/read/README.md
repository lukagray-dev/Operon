# operon-tools-fs-read

The `read` tool for the Operon agent's filesystem group. Reads one or multiple files in a single tool call with support for chunked reading, binary detection, and per-file error reporting.

## Features

- **Multi-file batched reads**: Read multiple files concurrently in a single tool call
- **Chunked reading**: Use `start_line`/`end_line` to read large files in manageable chunks
- **Binary file detection**: Automatically detects and rejects binary files (images, executables, etc.)
- **Size limits**: Enforces a 1 MB limit on full-file reads to prevent memory issues
- **Per-file error handling**: Individual file failures don't fail the entire tool call
- **Flexible input format**: Accepts both plain path strings and objects with line ranges

## Architecture

This crate is a **pure tool implementation** with zero dependencies on the Operon context pipeline. It only depends on:

- `operon-context-normalize-tools` — for canonical tool types (`ToolDefinition`, `ToolResult`, etc.)
- Standard async runtime (`tokio`) and serialization (`serde`, `serde_json`)

### File Structure

```
src/
├── lib.rs        ← Public API, re-exports, tool definition
├── args.rs       ← ReadArgs + ReadTarget deserialization
├── output.rs     ← ReadOutput, FileReadResult (per-file result type)
├── error.rs      ← ReadToolError (thiserror)
├── executor.rs   ← async execute(args) -> ToolResult, all tokio::fs logic
└── tests.rs      ← Comprehensive test suite
```

## Usage

### Basic Example

```rust
use operon_tools_fs_read::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    // 1. Get the tool definition to register with the model
    let def = definition();
    
    // 2. When the model calls the tool, execute it
    let args = json!({
        "paths": ["src/main.rs", "Cargo.toml"]
    });
    
    let result = execute(
        ToolCallId("call_123".to_string()),
        args
    ).await.unwrap();
    
    // 3. The result contains per-file success/error information
    println!("{:?}", result);
}
```

### Reading with Line Ranges

```rust
let args = json!({
    "paths": [{
        "path": "large_file.txt",
        "start_line": 100,
        "end_line": 200
    }]
});

let result = execute(call_id, args).await.unwrap();
```

### Mixed Input Formats

```rust
let args = json!({
    "paths": [
        "small_file.txt",  // Plain string — reads entire file
        {
            "path": "large_file.txt",
            "start_line": 1,
            "end_line": 100
        }
    ]
});
```

## Tool Definition

The tool exposes the following JSON Schema to the model:

```json
{
  "name": "read",
  "description": "Reads one or multiple files in a single call...",
  "parameters": {
    "type": "object",
    "properties": {
      "paths": {
        "type": "array",
        "items": {
          "oneOf": [
            { "type": "string" },
            {
              "type": "object",
              "properties": {
                "path": { "type": "string" },
                "start_line": { "type": "integer" },
                "end_line": { "type": "integer" }
              },
              "required": ["path"]
            }
          ]
        },
        "minItems": 1
      }
    },
    "required": ["paths"]
  }
}
```

## Output Format

The tool returns a `ToolResult` with `is_error: false` even when individual files fail. Per-file errors are embedded in the JSON content:

```json
{
  "files": [
    {
      "path": "success.txt",
      "success": true,
      "content": "file contents here",
      "total_lines": 42
    },
    {
      "path": "missing.txt",
      "success": false,
      "error": "Failed to access file: No such file or directory"
    }
  ]
}
```

### Line Range Output

When reading with a line range, the output includes `lines_returned`:

```json
{
  "files": [
    {
      "path": "file.txt",
      "success": true,
      "content": "lines 10-20 here",
      "total_lines": 100,
      "lines_returned": {
        "start": 10,
        "end": 20
      }
    }
  ]
}
```

## Error Handling

### Top-Level Errors

Returns `Err(ReadToolError::ArgsParse)` only if the JSON arguments are malformed (e.g., missing `paths` field).

### Per-File Errors

Individual file failures are captured in `FileReadResult`:

- **File not found**: `"Failed to access file: ..."`
- **Size limit exceeded**: `"File exceeds 1 MB limit (X bytes). Use start_line/end_line to read in chunks."`
- **Binary file**: `"Binary file detected. Use the image/video tool for media files."`
- **Invalid line range**: `"start_line X exceeds file length (Y lines)."`

## Size Limits

- **Full-file reads**: Maximum 1 MB (1,048,576 bytes)
- **Line-range reads**: No size limit (the agent explicitly requests a slice)

## Binary Detection

Files containing null bytes (`\0`) are rejected as binary. This prevents accidentally loading images, executables, or other non-text files.

## Concurrency

All file reads within a single tool call are executed concurrently using `futures::future::join_all` for optimal performance.

## Testing

Run the comprehensive test suite:

```bash
cargo test -p operon-tools-fs-read
```

Run the example:

```bash
cargo run --example basic_usage -p operon-tools-fs-read
```

## Design Constraints

- **No `unwrap()` or `expect()`** in production paths — all errors are handled explicitly
- **No `println!`** — use `tracing` if logging is needed
- **Zero context pipeline dependencies** — only depends on `operon-context-normalize-tools`
- **All public types derive `Debug`**
- **Serializable types derive `Serialize`/`Deserialize`**

## License

AGPL-3.0 — See the workspace root for details.
