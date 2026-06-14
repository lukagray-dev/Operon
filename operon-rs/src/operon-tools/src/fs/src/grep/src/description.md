Use `grep` tool to search for regex patterns in file contents or list files matching a glob pattern.

Format:

```example
<grep path="absolute\path\to\directory" pattern="regex_pattern" glob="glob_pattern" ignore="pattern1" "pattern2" context="lines">
```

Constraints & Usage:

- `path` (or `paths`): Must be an absolute directory or file path to search under.
- `pattern` (or `patterns`): Space-separated regex patterns to search for. Multiple patterns are OR-combined (match if any matches). If omitted, lists all matching files under path without searching content (glob-only mode).
- `glob`: Optional glob pattern to restrict which files are searched (e.g., "*.rs" or "*.py").
- `ignore`: Optional filename or directory name patterns to exclude from search.
- `context` (or `context_lines`): Optional number of lines of context before/after each match (default: 0).
- Results are capped at 300 matches.
