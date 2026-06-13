Deletes a file or recursively deletes a directory.

Format:
<delete path="[absolute_path]">
<<<<
permanent="[true_or_false]"
>>>>

Constraints & Usage:
- `path` must be an absolute path to an existing file or directory.
- `permanent` (body parameter): `"true"` permanently deletes the target from disk (irreversible). `"false"` or omitted moves the target to the Recycle Bin/Trash.
- Use Recycle Bin/Trash (default) whenever possible to prevent accidental data loss.
