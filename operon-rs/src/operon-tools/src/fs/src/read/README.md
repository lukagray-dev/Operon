# operon-tools-fs-read

The `read` tool for the Operon agent's filesystem group. Reads one or multiple files in a single tool call with support for inline string line ranges (`"file.rs:10-40"`, `"file.rs:5-EOF"`), absolute path enforcement, binary detection, and raw plain text output (`ToolContent::Text`).

## Features

- **Inline string line ranges**: Specify ranges directly in path strings (`"src/main.rs:10-40"`, `"src/lib.rs:5-EOF"`, `"src/args.rs:15"`)
- **Single file & batch reading**: Accepts top-level string `path` or `paths` array
- **Absolute path enforcement**: Requires absolute paths to avoid working directory ambiguity
- **Token-optimized plain text output**: Omits per-line line numbers in `read` responses to reduce token overhead
- **Binary file detection**: Automatically detects and rejects binary files (images, executables, etc.)
- **Size limits**: Enforces a 1 MB limit on full-file reads

## Usage

```rust
use operon_tools_fs_read::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    let def = definition();

    // Batch read with inline line ranges
    let args = json!({
        "paths": [
            "/home/user/project/src/main.rs:10-40",
            "/home/user/project/src/lib.rs:5-EOF"
        ]
    });

    let result = execute(ToolCallId("call_123".to_string()), args).await.unwrap();
    println!("{:?}", result);
}
```

## Output Format

Returns plain text (`ToolContent::Text`) with section headers and raw content:

```text
=== /home/user/project/src/main.rs (lines 10-40 of 200) ===
fn main() {
    println!("Hello, world!");
}
```

## Testing

```bash
cargo test -p operon-tools-fs-read
```

