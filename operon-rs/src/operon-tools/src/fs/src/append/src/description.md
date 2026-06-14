`append` tool appends text content to the end of an existing file. It does not create new files. Use `write` tool to create new files.

**How to use `append` tool:**

```example
<append path="absolute\path\to\file.txt">
<<<<
content to append
>>>>
```

* **`path` must be an absolute path to an existing file**
* **Content to append must be written in between `<<<<` `>>>>`**
* **Do not use escape `\n` for new lines, use real line breaks instead**
* **The content in the body is appended verbatim to the EOF. Include a leading newline if you want the appended content to start on a new line**