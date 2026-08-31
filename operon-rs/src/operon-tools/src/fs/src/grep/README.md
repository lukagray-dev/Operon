# operon-tools-fs-grep

The `grep` tool for the Operon agent's filesystem group. Searches files and directories for regex patterns with support for recursive directory walking, filename filtering, context lines, and plain-text output (`ToolContent::Text`).

## Features

- **Regex pattern matching**: Full Rust regex syntax support with case-sensitive/insensitive modes
- **Single or multiple target paths**: Accepts top-level `path` as a single path string or an array of path strings (`path: ["src", "tests"]`)
- **Recursive directory walking**: Automatically walks directories respecting `.gitignore`
- **Filename glob filtering**: Use `include` patterns like `*.rs` or `*.{ts,tsx}` during directory walks
- **Context lines**: Default 2 lines before/after matches (customizable via `context_lines`)
- **Plain text formatting**: Grouped by file headers with line numbers and `---` block separators
- **Safety limits**: Caps results at 300 matches and skips files > 10 MB or binary content

## Usage

```rust
use operon_tools_fs_grep::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    let def = definition();

    let args = json!({
        "pattern": "fn main",
        "path": ["/home/user/project/src", "/home/user/project/tests"]
    });

    let result = execute(ToolCallId("call_123".to_string()), args).await.unwrap();
    println!("{:?}", result);
}
```

## Output Format

Returns plain text (`ToolContent::Text`) grouped by file header with `---` block separators:

```text
=== /home/user/project/src/main.rs (1 match) ===
10: fn main() {
11:     println!("Hello");

Showing 1 match(es) across 1 file(s).
```

## Testing

```bash
cargo test -p operon-tools-fs-grep
```

