`edit` tool applies precise, diff-based modifications (hunks) to an existing file. Use `edit` tool to modify specific sections of a file without overwriting the entire file. You can apply multiple edits (hunks) at once using this tool.

**How to use `edit` tool:**

```example
<edit path="absolute\path\to\file.txt">
<<<<
@@ [optional_context_header]
 [context_line_matching_file_exactly]
-[line_to_remove]
+[line_to_add]
 [context_line_matching_file_exactly]
>>>>
```

### Usage Rules & Guidelines

* **`path` must be an absolute path to an existing file**
* **Content to edit must be written in between `<<<<` `>>>>`**
* **Before editing a file, you must read it in that turn**. The read ledger will block edits if the file has not been read recently
* **Diff lines are prefix-coded**:
  - ` ` (single space) prefix for unchanged context lines (critical for locating the edit).
  - `-` (minus) prefix for lines to remove.
  - `+` (plus) prefix for lines to insert or replace.
* **Context lines must match the file exactly** (indentation, tabs, and trailing spaces must match perfectly).
* **Handling Empty/Blank Lines**:
  - Do not write a blank line `""` (with no prefix) to represent an empty line in a hunk; the parser skips empty lines as visual separators.
  - To represent a blank line:
    - Use ` ` (a single space on its own line) for blank context.
    - Use `-` (a single hyphen on its own line) to remove a blank line.
    - Use `+` (a single plus on its own line) to insert a blank line.
* **EOF Anchor**:
  - To anchor an edit at the end of the file, add `@@ EOF` or `-*** End of File` on its own line at the end of the hunk.

---

### Examples

#### Example 1: Basic replacement (replacing a function name)

```example
<edit path="D:\Project Operon\Operon\src\main.rs">
<<<<
@@ Rename hello function
 fn old_hello() {
-    println!("hello");
+    println!("hello world!");
 }
>>>>
```

#### Example 2: Pure Insertion (inserting imports at the top of a file)

```example
<edit path="D:\Project Operon\Operon\src\lib.rs">
<<<<
@@ Insert serde import
+use serde::{Serialize, Deserialize};
+
 fn run() {}
>>>>
```

#### Example 3: Multiple hunks in a single tool call

```example
<edit path="D:\Project Operon\Operon\src\main.rs">
<<<<
@@ Update imports
-import { oldFunc } from './lib';
+import { newFunc } from './lib';
@@ Call new function
 fn main() {
-    oldFunc();
+    newFunc();
 }
>>>>
```

#### Example 4: Multi-hunk edit handling blank lines
```example
<edit path="D:\Project Operon\Operon\README.md">
<<<<
@@ Remove obsolete paragraph
-This paragraph is obsolete and should be removed.
 
 # Section 2
@@ Update Section 2 content
 # Section 2
-This is the old text.
+This is the new text.
>>>>
```
*(Note the single space prefix on the blank line before `# Section 2` in the first hunk to represent the empty context line).*
