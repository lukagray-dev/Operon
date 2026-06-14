`ls` tool lists files and directories under the specified path. Use `ls` tool to find files and directories, explore directory structures, or search for files matching a pattern.

**How to use `ls` tool:**

```example
<ls path="absolute\path\to\directory" depth="integer" glob="glob_pattern" ignore="pattern1" "pattern2">
```

* **`path` (or `paths`): Must be an absolute directory path**
* **`depth`: Integer directory recursion depth (default: 1, use 0 for unlimited recursion)**
* **`glob`: Optional glob pattern to filter matching file names (e.g., "*.rs")**
* **`ignore`: Optional filename or directory name patterns to skip (e.g., "node_modules", ".git")**
* **Output includes directory structure and file sizes. Capped at 1000 entries**
