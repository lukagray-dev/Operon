# operon-tools-web-fetch

Web fetch tool for the Operon agent. Fetches URLs and returns page content as clean markdown.

## Overview

`web_fetch` enables the Operon agent to fetch URLs and retrieve page content as clean markdown. The tool strips navigation, ads, and boilerplate, extracts page titles, and handles HTTP error statuses gracefully.

**Key characteristics:**
- HTTP and HTTPS support
- HTML→markdown conversion via htmd
- Title extraction from `<title>` tag
- Content truncation at 20,000 characters
- HTTP error statuses (4xx, 5xx) returned as structured output, not errors
- Configurable timeout (default 15 seconds)
- Static content only — no JavaScript-rendered pages

## Usage

### Basic fetch

```rust
use operon_tools_web_fetch::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "url": "https://www.rust-lang.org"
        })
    ).await.unwrap();
    
    println!("{:?}", result);
}
```

### With custom timeout

```rust
let result = execute(
    ToolCallId("call_2".to_string()),
    json!({
        "url": "https://example.com/large-page",
        "timeout_ms": 30000  // 30 seconds
    })
).await.unwrap();
```

## Output format

```json
{
  "url": "https://www.rust-lang.org",
  "status_code": 200,
  "title": "Rust Programming Language",
  "content": "# Rust Programming Language\n\nA language empowering everyone...",
  "truncated": false,
  "content_length": 5432
}
```

### Output fields

| Field | Type | Description |
|-------|------|-------------|
| `url` | string | The URL that was fetched (may differ from input if redirected) |
| `status_code` | integer | HTTP status code (200, 404, 500, etc.) |
| `title` | string \| null | Page title from `<title>` tag, if present |
| `content` | string | Page content as clean markdown, truncated to 20,000 characters |
| `truncated` | boolean | True if content was truncated at 20,000 characters |
| `content_length` | integer | Content length in characters (after truncation) |

## Error handling

| Condition | Response |
|-----------|----------|
| Empty URL | `is_error: true`, text: "url is empty" |
| Invalid scheme (not http/https) | `is_error: true`, text: "url must start with http:// or https://" |
| Network failure (DNS, timeout, connection refused) | `is_error: true`, text: "fetch failed: ..." |
| HTTP error status (4xx, 5xx) | `is_error: false`, `status_code: 404`, `content: ""` |
| Success | `is_error: false`, JSON output with content |

**Important**: HTTP error statuses (404, 403, 500) are **not** tool errors. The model receives the status code and can decide whether to retry, try a different URL, or report the error.

## HTTP status codes

| Status | Meaning | Action |
|--------|---------|--------|
| 200 | Success | Content is available in `content` field |
| 301/302/307 | Redirect | Automatically followed; `url` field shows final URL |
| 400 | Bad Request | URL may be malformed; try a different URL |
| 403 | Forbidden | Page is blocked or requires authentication |
| 404 | Not Found | Page does not exist; try a different URL or search |
| 500 | Server Error | Server is down; retry later or try a different source |

## Content truncation

If `truncated: true`, the full page content is longer than 20,000 characters.

**To get the relevant part:**
- Use a more specific URL (e.g., fetch the docs page directly, not the homepage)
- Use `web_search` with a more targeted query to find a more specific page
- Extract the section you need from the truncated content

**Example:**
```json
// Wrong: fetch homepage (likely to be truncated)
{"url": "https://example.com"}

// Right: fetch specific docs page
{"url": "https://example.com/docs/api-reference"}
```

## Title extraction

The `title` field contains the content of the `<title>` tag, if present. If the page has no `<title>` tag or the title is empty, `title` is `null`.

## HTML→markdown conversion

The content is converted from HTML to markdown using htmd, which:
- Strips navigation, ads, and boilerplate
- Converts headings, lists, links, code blocks, etc. to markdown
- Removes inline styles and scripts
- Preserves text content and structure

If conversion fails (rare), a fallback plain-text extraction is used.

## Limitations

- **Static content only**: JavaScript-rendered pages (SPAs, dynamic content) may return empty or partial content. The tool fetches the initial HTML — it does not execute JavaScript.
- **No authentication**: The tool does not support cookies, authentication headers, or login flows.
- **No redirects beyond HTTP**: The tool follows HTTP redirects but does not handle meta-refresh or JavaScript redirects.
- **Content cap**: Maximum 20,000 characters returned. Larger pages are truncated.

## Common workflow

1. Use `web_search` to find relevant URLs
2. Pick a promising result URL
3. Use `web_fetch` to read the full content of that URL
4. Extract the information needed from the fetched content

## Timeout behavior

The default timeout is 15 seconds. For slow sites or large pages, increase `timeout_ms`:

```json
{
  "url": "https://slow-site.example.com",
  "timeout_ms": 30000  // 30 seconds
}
```

If the request exceeds the timeout, the tool returns `is_error: true` with a timeout message.

## Testing

Run non-network tests (default):
```bash
cargo test -p operon-tools-web-fetch
```

Run network tests (requires internet):
```bash
cargo test -p operon-tools-web-fetch -- --ignored
```

Network tests include:
- `test_fetch_known_url` — Fetch https://www.rust-lang.org and verify content
- `test_fetch_404` — Verify 404 status is handled correctly
- `test_content_length_matches` — Verify content_length matches actual content
- `test_truncation_flag` — Verify truncation flag is set correctly
- `test_timeout_respected` — Verify timeout is enforced
- `test_redirect_followed` — Verify HTTP redirects are followed

## Implementation details

- Uses `reqwest` for HTTP requests with `rustls-tls` (no OpenSSL dependency)
- Uses `htmd` for HTML→markdown conversion
- Title extraction via simple char-by-char search (no full HTML parser)
- Fallback plain-text extraction if htmd conversion fails
- Content truncated at character boundary (not byte boundary)
- User-Agent header set to "Mozilla/5.0 (compatible; Operon/1.0)"

## Dependencies

- `reqwest` — HTTP client with rustls-tls
- `htmd` — HTML to markdown conversion
- `tokio` — Async runtime
- `serde_json` — JSON serialization
- `thiserror` — Error handling
- `operon-context-normalize-tools` — Tool integration types
- `operon-tools-core` — Shared tool types

## See also

- `web_search` — Search the web and find URLs
- `web_fetch` definition in dispatcher — Tool registration
