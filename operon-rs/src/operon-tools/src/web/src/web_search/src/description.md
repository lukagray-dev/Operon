`web_search` tool performs a web search via DuckDuckGo and returns list titles, URLs, and snippets. Use `web_search` tool to search for information on the web.

**How to use `web_search` tool:**

```example
<web_search query="search_query" max="number">
```

* **`query`: The search terms. DuckDuckGo operators (quotes for exact match, `site:`, etc.) are supported**
* **`max` (optional): Number of results to retrieve (default: 5, maximum: 10)**