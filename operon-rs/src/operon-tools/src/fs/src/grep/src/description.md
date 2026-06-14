`grep` tool searches for regex patterns in file contents or lists files matching a glob pattern. Use `grep` tool to search for patterns in file contents, filter files by name, or find files that match a specific pattern.

**How to use `grep` tool:**

```example
<grep path="absolute\path\to\directory" pattern="pattern1" "pattern2" "pattern3" glob="glob_pattern" ignore="pattern1" "pattern2" context="integer">
```

* **`path` (or `paths`): Must be an absolute directory or file path to search under**
* **`pattern` (or `patterns`): Space-separated regex patterns to search for. Multiple patterns are OR-combined (match if any matches). If omitted, lists all matching files under path without searching content (glob-only mode)**
* **`glob`: Optional glob pattern to restrict which files are searched (e.g., "*.rs" or "*.py")**
* **`ignore`: Optional filename or directory name patterns to exclude from search**
* **`context` (or `context_lines`): Optional number of lines of context before/after each match (default: 0)**