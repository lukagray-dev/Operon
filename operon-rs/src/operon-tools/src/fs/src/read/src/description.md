Reads the content of one or more files.

Format:
<read paths="[path1]" "[path2:start-end]">

Constraints & Usage:
- `paths` is a space-separated list of absolute file paths.
- Specify line ranges after a colon (e.g., `/path/to/file.rs:10-50` for lines 10 to 50 inclusive, `/path/to/file.rs:50-` for line 50 to end, `/path/to/file.rs:-30` for lines 1 to 30).
- Line ranges are 1-indexed and inclusive.
- Full-file reads (no range) are capped at 1 MB. Use line ranges for larger files.
- Output is printed with line numbers for reference.
