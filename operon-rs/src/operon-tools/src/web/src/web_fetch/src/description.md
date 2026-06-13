Fetches the raw content of a webpage and converts it into clean markdown.

Format:
<web_fetch url="[url]">

Constraints & Usage:
- `url` (required): Absolute URL starting with `http://` or `https://`.
- Headless browser rendering is used to execute client-side JavaScript.
- Boilerplate content (ads, nav bars, headers) is stripped automatically.
- Output is truncated to 10,000 characters.
