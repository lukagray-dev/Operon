Use `ls` tool to list files and directories under the specified path.

Format:

```example
<ls path="absolute\path\to\directory" depth="depth" glob="glob_pattern" ignore="pattern1" "pattern2">
```

Constraints & Usage:

- `path` (or `paths`): Must be an absolute directory path.
- `depth`: Directory recursion depth (default: 1, use 0 for unlimited recursion).
- `glob`: Optional glob pattern to filter matching file names (e.g., "*.rs").
- `ignore`: Optional filename or directory name patterns to skip (e.g., "node_modules", ".git").
- Output includes directory structure and file sizes. Capped at 1000 entries.
