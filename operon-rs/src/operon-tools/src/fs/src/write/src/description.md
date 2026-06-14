`write` tool creates a new file or completely overwrites an existing file at the absolute path specified. Use `write` tool to create new files or overwrite existing files.

**How to use `write` tool:**

```example
<write path="absolute\path\to\file.txt">
<<<<
content to write
>>>>
```

* **`path` must be an absolute path to an existing file**
* **Content to write must be written in between `<<<<` `>>>>`**
* **Do not use escape `\n` for new lines, use real line breaks instead**
* **Before overwriting an existing file, you must read that file in that turn. Read ledger will reject overwriting files if not read**
* **Parent directories will be created automatically if they do not exist**
* **To edit small parts of existing files, use `edit` tool instead**