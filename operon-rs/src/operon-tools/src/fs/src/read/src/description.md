`read` tool reads the content of one or more files in one tool call. Use `read` tool to read file contents, inspect file structures, or read specific code snippets.

**How to use `read` tool:**

```example
<read paths="absolute\path\to\file.txt" "absolute\path\to\another_file.rs:10-50">
```

* **`paths`**: Space-separated list of absolute file paths.
* **Specify line ranges after a colon** (e.g., `absolute\path\to\file.rs:10-50` for lines 10 to 50 inclusive, `absolute\path\to\file.rs:50-` for line 50 to end, `absolute\path\to\file.rs:-30` for lines 1 to 30).
* **Line ranges are 1-indexed and inclusive**
* **Full-file reads (no range) are capped at 1 MB. Use line ranges for larger files.**