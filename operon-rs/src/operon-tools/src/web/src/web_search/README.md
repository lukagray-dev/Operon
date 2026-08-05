# operon-tools-web-search

Web search tool for the Operon agent. Queries DuckDuckGo and returns structured search results with no API key required.

## Overview

`web_search` enables the Operon agent to search the web and retrieve structured results (title, URL, snippet) from DuckDuckGo. Results are returned as JSON with rank, title, URL, and snippet for each result.

**Key characteristics:**
- No API key required — uses DuckDuckGo's public lite search API
- Structured output — rank, title, URL, snippet for each result
- Configurable result count — 1–10 results (default 5)
- DuckDuckGo query syntax support — site:, filetype:, quotes, etc.
- Static content only — no JavaScript-rendered pages
- Privacy-respecting — DuckDuckGo does not track queries

## Usage

### Basic search

```rust
use operon_tools_web_search::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "query": "rust programming language"
        })
    ).await.unwrap();
    
    println!("{:?}", result);
}
```

### With result limit

```rust
let result = execute(
    ToolCallId("call_2".to_string()),
    json!({
        "query": "machine learning",
        "max_results": 3
    })
).await.unwrap();
```

### Advanced query syntax

```rust
// Site search
json!({"query": "site:github.com rust async"})

// File type search
json!({"query": "filetype:pdf neural networks"})

// Exact phrase
json!({"query": "\"machine learning\" -deprecated"})

// Combined
json!({"query": "site:github.com rust -deprecated async"})
```

## Output format

```json
{
  "query": "rust programming language",
  "result_count": 5,
  "results": [
    {
      "rank": 1,
      "title": "The Rust Programming Language",
      "url": "https://www.rust-lang.org",
      "snippet": "A language empowering everyone to build reliable and efficient software."
    },
    {
      "rank": 2,
      "title": "Rust - Wikipedia",
      "url": "https://en.wikipedia.org/wiki/Rust_(programming_language)",
      "snippet": "Rust is a multi-paradigm, general-purpose programming language..."
    }
  ]
}
```

## Error handling

| Condition | Response |
|-----------|----------|
| Empty query | `is_error: true`, text: "query is empty" |
| Network failure | `is_error: true`, text: "search failed: ..." |
| No results found | `is_error: false`, `result_count: 0`, `results: []` |
| Success | `is_error: false`, JSON output with results |

Empty results are **not** an error — the model receives an empty results array and can refine the query or try a different approach.

## Query syntax

DuckDuckGo supports the following operators:

| Operator | Example | Meaning |
|----------|---------|---------|
| Quotes | `"exact phrase"` | Search for exact phrase |
| Site | `site:github.com` | Search within a specific domain |
| File type | `filetype:pdf` | Search for specific file types |
| Exclude | `-keyword` | Exclude results containing keyword |
| Combine | `site:github.com rust -deprecated` | Combine multiple operators |

## Limitations

- **Static content only**: JavaScript-rendered pages (SPAs, dynamic content) may return empty or partial snippets. The tool fetches static HTML only.
- **Snippet length**: Snippets are short (typically 100–200 characters). Use `web_fetch` to read the full page content.
- **Result cap**: Maximum 10 results per query. More results rarely improve outcomes and increase token usage.
- **No authentication**: The tool does not support authenticated searches or personalized results.

## Common workflow

1. Use `web_search` to find relevant URLs
2. Pick a promising result URL
3. Use `web_fetch` to read the full content of that URL
4. Extract the information needed from the fetched content

## Testing

Run non-network tests (default):
```bash
cargo test -p operon-tools-web-search
```

Run network tests (requires internet):
```bash
cargo test -p operon-tools-web-search -- --ignored
```

Network tests include:
- `test_basic_search` — Query "rust programming language" and verify results
- `test_max_results_respected` — Verify result count limit is enforced
- `test_max_results_cap_enforced` — Verify cap at 10 results
- `test_no_results` — Handle queries with no results
- `test_site_search` — Test DuckDuckGo site: syntax

## Implementation details

- Uses the `duckduckgo` crate's `lite_search()` API for structured results
- Executes searches in `tokio::task::spawn_blocking` (DuckDuckGo crate uses blocking runtime internally)
- Results are 1-indexed (first result has rank 1)
- Queries are trimmed and validated before execution
- Result count is capped at 10 to manage token usage

## Dependencies

- `duckduckgo` — DuckDuckGo search API
- `tokio` — Async runtime
- `serde_json` — JSON serialization
- `thiserror` — Error handling
- `operon-context-normalize-tools` — Tool integration types
- `operon-tools-core` — Shared tool types

## See also

- `web_fetch` — Fetch and read full page content
- `web_search` definition in dispatcher — Tool registration
