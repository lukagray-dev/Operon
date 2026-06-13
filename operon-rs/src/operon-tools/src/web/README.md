# operon-tools-web

Web tools for the Operon agent. Enables web search and content fetching.

## Overview

The `operon-tools-web` crate provides a unified interface to web-related tools for the Operon agent:

- **`web_search`** — Query DuckDuckGo and retrieve structured search results
- **`web_fetch`** — Fetch URLs and retrieve page content as clean markdown

These tools enable the agent to research topics, find information, and read web content as part of its reasoning and decision-making process.

## Architecture

```
operon-tools-web (facade)
├── web_search (search tool)
│   ├── args.rs (input validation)
│   ├── executor.rs (DuckDuckGo integration)
│   ├── output.rs (result types)
│   ├── error.rs (error types)
│   └── lib.rs (tool definition & entry point)
└── web_fetch (fetch tool)
    ├── args.rs (input validation)
    ├── executor.rs (HTTP fetching & HTML→markdown)
    ├── output.rs (result types)
    ├── error.rs (error types)
    └── lib.rs (tool definition & entry point)
```

## Quick start

### Register web tools with the dispatcher

```rust
use operon_tools::dispatcher::Dispatcher;

let mut dispatcher = Dispatcher::new();
dispatcher.register_fs_tools();
dispatcher.register_shell_tools();
dispatcher.register_web_tools();  // Register web tools
dispatcher.register_todo_tools();

// Get tool definitions to send to the model
let defs: Vec<_> = dispatcher.definitions().collect();
```

### Use web_search

```rust
use operon_tools_web::web_search;
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

let result = web_search::execute(
    ToolCallId("call_1".to_string()),
    json!({
        "query": "rust programming language",
        "max_results": 5
    })
).await.unwrap();
```

### Use web_fetch

```rust
use operon_tools_web::web_fetch;
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

let result = web_fetch::execute(
    ToolCallId("call_2".to_string()),
    json!({
        "url": "https://www.rust-lang.org",
        "timeout_ms": 15000
    })
).await.unwrap();
```

## Tool definitions

### web_search

Searches DuckDuckGo and returns structured results (title, URL, snippet).

**Input:**
```json
{
  "query": "search query",
  "max_results": 5
}
```

**Output:**
```json
{
  "query": "search query",
  "result_count": 5,
  "results": [
    {
      "rank": 1,
      "title": "Result title",
      "url": "https://example.com",
      "snippet": "Short description..."
    }
  ]
}
```

**Key features:**
- No API key required
- DuckDuckGo query syntax support (site:, filetype:, quotes, etc.)
- Configurable result count (1–10, default 5)
- Static content only (no JavaScript-rendered pages)
- Privacy-respecting (DuckDuckGo does not track queries)

See [`web_search/README.md`](src/web_search/README.md) for detailed documentation.

### web_fetch

Fetches URLs and returns page content as clean markdown.

**Input:**
```json
{
  "url": "https://example.com",
  "timeout_ms": 15000
}
```

**Output:**
```json
{
  "url": "https://example.com",
  "status_code": 200,
  "title": "Page title",
  "content": "# Page title\n\nPage content as markdown...",
  "truncated": false,
  "content_length": 5432
}
```

**Key features:**
- HTTP and HTTPS support
- HTML→markdown conversion
- Title extraction from `<title>` tag
- Content truncation at 20,000 characters
- HTTP error statuses (4xx, 5xx) returned as structured output
- Configurable timeout (default 15 seconds)
- Static content only (no JavaScript-rendered pages)

See [`web_fetch/README.md`](src/web_fetch/README.md) for detailed documentation.

## Common workflows

### Research a topic

1. Use `web_search` to find relevant URLs
2. Pick a promising result
3. Use `web_fetch` to read the full content
4. Extract information from the fetched content

```rust
// Step 1: Search
let search_result = web_search::execute(
    ToolCallId("search_1".to_string()),
    json!({"query": "rust async programming"})
).await?;

// Step 2: Pick a result (e.g., first result)
let url = /* extract URL from search_result */;

// Step 3: Fetch
let fetch_result = web_fetch::execute(
    ToolCallId("fetch_1".to_string()),
    json!({"url": url})
).await?;

// Step 4: Extract information
let content = /* extract content from fetch_result */;
```

### Handle truncated content

If `web_fetch` returns `truncated: true`, the content was capped at 20,000 characters.

**Options:**
1. Use a more specific URL (e.g., fetch the docs page directly, not the homepage)
2. Use `web_search` with a more targeted query to find a more specific page
3. Extract the section you need from the truncated content

### Handle HTTP errors

If `web_fetch` returns a non-2xx status code, the model receives the status and can decide:

```rust
let result = web_fetch::execute(...).await?;
match result.content {
    ToolContent::Text(text) => {
        let output: WebFetchOutput = serde_json::from_str(&text)?;
        match output.status_code {
            200 => { /* success */ },
            404 => { /* not found — try different URL */ },
            403 => { /* forbidden — may need authentication */ },
            500 => { /* server error — retry later */ },
            _ => { /* other error */ }
        }
    }
}
```

## Error handling

Both tools follow the same error handling pattern:

| Condition | Response |
|-----------|----------|
| Malformed arguments | `is_error: true`, text error from dispatcher |
| Validation failure (empty query/URL, invalid scheme) | `is_error: true`, text error |
| Network failure | `is_error: true`, text error |
| HTTP error status (4xx, 5xx) | `is_error: false`, structured output with status code |
| Success | `is_error: false`, JSON output |

**Important**: HTTP error statuses are **not** tool errors. The model receives the status code and can decide how to proceed.

## Limitations

### web_search

- Static content only — no JavaScript-rendered pages
- Snippet length — snippets are short (100–200 characters), use `web_fetch` for full content
- Result cap — maximum 10 results per query
- No authentication — no support for authenticated searches

### web_fetch

- Static content only — no JavaScript-rendered pages (SPAs won't work)
- No authentication — no support for cookies or login flows
- No meta-refresh or JavaScript redirects — only HTTP redirects
- Content cap — maximum 20,000 characters returned

## Testing

Run all web tool tests:
```bash
cargo test -p operon-tools-web
cargo test -p operon-tools-web-search
cargo test -p operon-tools-web-fetch
```

Run network tests (requires internet):
```bash
cargo test -p operon-tools-web-search -- --ignored
cargo test -p operon-tools-web-fetch -- --ignored
```

## Dependencies

### External crates

- `duckduckgo` — DuckDuckGo search API
- `reqwest` — HTTP client with rustls-tls
- `htmd` — HTML to markdown conversion
- `tokio` — Async runtime
- `serde_json` — JSON serialization
- `thiserror` — Error handling

### Internal crates

- `operon-context-normalize-tools` — Tool integration types
- `operon-tools-core` — Shared tool types

## Integration with dispatcher

Web tools are registered with the dispatcher via `register_web_tools()`:

```rust
pub fn register_web_tools(&mut self) {
    self.register(
        operon_tools_web_search::definition(),
        |call_id, args| async move {
            operon_tools_web_search::execute(call_id, args)
                .await
                .map_err(|e| e.to_string())
        },
    );
    self.register(
        operon_tools_web_fetch::definition(),
        |call_id, args| async move {
            operon_tools_web_fetch::execute(call_id, args)
                .await
                .map_err(|e| e.to_string())
        },
    );
}
```

## See also

- [`web_search/README.md`](src/web_search/README.md) — Detailed web_search documentation
- [`web_fetch/README.md`](src/web_fetch/README.md) — Detailed web_fetch documentation
- `operon-tools` — Main tools crate
- `operon-tools-core` — Shared tool types
