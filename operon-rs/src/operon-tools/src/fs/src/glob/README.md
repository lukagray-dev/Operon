# operon-tools-fs-glob

The `glob` tool for the Operon agent's filesystem group. Finds files and directories matching wildcard glob patterns (e.g. `**/*.rs`, `src/**/*.tsx`, `*.json`) across the workspace with gitignore awareness, recursion, and plain text formatting.

## Features

- **Wildcard glob matching**: Supports full glob pattern syntax (`*`, `**`, `?`, `[...]`) via `globset`
- **Gitignore-aware traversal**: Powered by ripgrep's `ignore` engine, automatically skipping `.gitignore` matches, hidden files, and build artifacts
- **Configurable result limit**: Default 100 matches, up to a safety ceiling of 1000 matches (`max_results`)
- **Forward-slash normalized paths**: Outputs alphabetically sorted relative paths with consistent `/` separators across all platforms
- **Plain text formatting**: Clear banner header with pattern, base directory, match count, and truncation indicator

## Usage

```rust
use operon_tools_fs_glob::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    let def = definition();

    let args = json!({
        "pattern": "**/*.rs",
        "path": "/home/user/project"
    });

    let result = execute(ToolCallId("call_123".to_string()), args).await.unwrap();
    println!("{:?}", result);
}
```

## Output Format

Returns plain text (`ToolContent::Text`):

```text
=== glob("**/*.rs") in /home/user/project (3 match(es)) ===
src/lib.rs
src/main.rs
tests/integration_test.rs
```

## Testing

```bash
cargo test -p operon-tools-fs-glob
```

