Use `ls` tool to list files and directories under the specified path.

Format (Attributes - preferred for simple/single-line arguments):

```example
<ls path="absolute\path\to\directory" depth="depth" glob="glob_pattern" ignore="pattern1" "pattern2">
```

Format (Body - optional):

```example
<ls path="absolute\path\to\directory">
<<<<
depth="depth"                         // default is 1. You can use 0 for unlimited recursion.
glob="glob_pattern"                   // optional glob pattern to filter matching file names (e.g., "*.rs").
ignore="pattern1" "pattern2"          // optional filename or directory name patterns to skip (e.g., "node_modules", ".git").
>>>>
```

Constraints & Usage:

- `path` (or `paths`): Must be an absolute directory path.
- `depth`: Directory recursion depth (default: 1, use 0 for unlimited recursion).
- `glob`: Optional glob pattern to filter matching file names (e.g., "*.rs").
- `ignore`: Optional filename or directory name patterns to skip (e.g., "node_modules", ".git").
- Output includes directory structure and file sizes. Capped at 1000 entries.
