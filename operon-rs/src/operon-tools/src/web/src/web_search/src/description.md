Performs a web search via DuckDuckGo and returns list titles, URLs, and snippets.

Format:
<web_search query="[search_query]" max="[number]">

Constraints & Usage:
- `query` (required): The search terms. DuckDuckGo operators (quotes for exact match, `site:`, etc.) are supported.
- `max` (optional): Number of results to retrieve (default: 5, maximum: 10).
- Use `web_fetch` on returned URLs to retrieve full page content.
