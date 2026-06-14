`delete` tool deletes a file or recursively deletes a directory. Use it to remove files or directories.

**How to use `delete` tool:**

```example
<delete path="absolute\path\to\file_or_directory" permanent="true_or_false">
```

* **`path` must be an absolute path to an existing file or directory**
* **`permanent` (optional attribute): `"true"` permanently deletes the target from disk (irreversible). `"false"` or omitted moves the target to the Recycle Bin**
* **Use Recycle Bin (default) whenever possible to prevent accidental data loss**
