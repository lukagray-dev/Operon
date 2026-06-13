Appends text content to the end of an existing file.

Format:
<append path="[absolute_path]">
<<<<
[content to append]
>>>>

Constraints & Usage:
- `path` must be an absolute path to an existing file. This tool will NOT create new files (use the `write` tool first).
- The content in the body is appended verbatim to the EOF. Include a leading newline if you want the appended content to start on a new line.
