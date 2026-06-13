Creates a new file or completely overwrites an existing file at the absolute path specified.

Format:
<write path="[absolute_path]">
<<<<
[file content]
>>>>

Constraints & Usage:
- `path` must be an absolute path.
- Parent directories will be created automatically if they do not exist.
- Content must be written inside the `<<<<` and `>>>>` delimiter block.
- To edit small parts of an existing file, do NOT use this tool; use the `edit` tool instead to save token context.
