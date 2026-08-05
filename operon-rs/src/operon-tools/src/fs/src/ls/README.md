# operon-tools-fs-ls

The `ls` tool for the Operon agent's filesystem group. Provides single-level directory listing with entry type classification, metadata collection, and glob-pattern exclusion in plain-text format (`ToolContent::Text`).

## Features

- **Optional path parameter**: Defaults to `"."` (current directory) if `path` is omitted
- **Flexible aliases**: Accepts `path` or `dir`
- **Glob-pattern exclusion**: Filter entries by name using glob patterns (e.g. `*.lock`, `node_modules`)
- **Plain text formatting**: Clear columnar layout (`[DIR] folder/`, `[FILE] name (1.2 KB)`)
- **Truncation handling**: Caps results at 1000 entries to prevent context overflow

## Usage

```rust
use operon_tools_fs_ls::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    let def = definition();

    let args = json!({
        "path": "/home/user/project",
        "ignore": ["*.lock", "node_modules", ".git"]
    });

    let result = execute(ToolCallId("call_123".to_string()), args).await.unwrap();
    println!("{:?}", result);
}
```

## Output Format

Returns plain text (`ToolContent::Text`):

```text
=== /home/user/project (3 items) ===
[DIR]  src/
[FILE] Cargo.toml (1.0 KB)
[FILE] README.md (2.0 KB)
```

## Testing

```bash
cargo test -p operon-tools-fs-ls
```

