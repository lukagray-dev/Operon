Searches for memories. If the query is empty or omitted, it loads all stored memories.

Format:
<memory_search query="[search_query]">

Constraints & Usage:
- `query` (optional): The search string to query against memory contents. If empty or omitted (e.g., query=""), all memories in the database will be loaded. Use short, distinct keywords or key phrases relevant to what you want to recall (e.g., `api key`, `project setup`, `deploy command`).
- The search is case-insensitive and performs a substring match (matching anything containing the query), so a single keyword often works best.

