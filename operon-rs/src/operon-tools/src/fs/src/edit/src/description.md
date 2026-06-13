Applies precise, diff-based modifications (hunks) to an existing file.

Format:
<edit path="[absolute_path]">
<<<<
@@ [optional_context_header]
 [context line exactly matching file]
-[lines to remove]
+[lines to add]
 [context line exactly matching file]
>>>>

Constraints & Usage:
- `path` must be an absolute path to an existing file.
- Diff body consists of one or more hunks starting with `@@`.
- Prefix lines to remove with `-`, lines to insert/replace with `+`, and unchanged context lines with a single space.
- Context lines must match the existing file content exactly (whitespace and indentation are critical).
- This is atomic: either all hunks apply successfully or the file remains untouched.
