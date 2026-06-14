`web_fetch` tool fetches the raw content of a webpage and converts it into clean markdown. Use `web_fetch` tool to fetch the content of a webpage.

**How to use `web_fetch` tool:**

```example
<web_fetch url="url">
```

**Constraints & Usage:**

* **`url`: Absolute URL starting with `http://` or `https://`**
* **Headless browser rendering is used to execute client-side JavaScript**
* **Boilerplate content (ads, nav bars, headers) is stripped automatically**