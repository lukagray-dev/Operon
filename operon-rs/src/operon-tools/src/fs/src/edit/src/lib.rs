//! # operon-tools-fs-edit
//!
//! Implements the `edit` tool for the Operon agent's filesystem group.
//!
//! Edits an existing file using a diff-based body format. Supports:
//! - Multi-hunk edits (one or more `@@`-delimited hunks per call)
//! - Fuzzy line matching via `seek_sequence` (exact → rstrip → trim → unicode-normalised)
//! - Optional per-hunk seek anchors (`@@ some context line`)
//! - EOF-anchored hunks (`*** End of File` marker)
//! - Overlap detection across hunks
//! - Atomic writes (all hunks applied or none)
//!
//! ## Call format
//!
//! ```text
//! <edit path="C:\absolute\path\to\file.rs">
//! <<<<
//! @@
//! -old line
//! +new line
//! >>>>
//! ```
//!
//! The dispatcher injects:
//! - `args_json["path"]`     — the absolute file path from the `path` XML attr.
//! - `args_json["__body__"]` — the raw diff body between `<<<<` and `>>>>`.

mod args;
mod error;
mod executor;
mod seek_sequence;

#[cfg(test)]
mod tests;

// Keep EditArgs accessible to callers that construct it directly (e.g. integration tests).
// EditHunk is internal — callers only interact with EditArgs.
pub use args::EditArgs;
pub use error::EditToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `edit` tool.
///
/// - `short`:    sent to the model under normal conditions. Concise summary of
///               the tool's purpose and the diff body format.
/// - `detailed`: sent after a malformed call. Full explanation of the call
///               format, hunk syntax, error messages, and worked examples.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "edit".to_string(),
            description: "Edits an existing file using a diff-based body format. \
                          path attr is the absolute file path. \
                          Write the diff in the tool body using @@ hunk separators, \
                          - for lines to remove, + for lines to add, \
                          and a space prefix for context lines used to locate the edit position. \
                          Multiple hunks per call are supported."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "edit".to_string(),
            description: "\
Edits an existing file using a diff-based body format. All hunks are applied atomically — \
if any hunk fails, the file is NOT modified.

## Call format

```
<edit path=\"C:\\\\absolute\\\\path\\\\to\\\\file.rs\">
<<<<
@@
-old line one
-old line two
+new line one
+new line two
@@
-another old line
+another new line
>>>>
```

The `path` attr is the absolute path to the file to edit.
The body between `<<<<` and `>>>>` contains one or more hunks separated by `@@`.

## Hunk syntax

Each line in the body must start with one of:

| Prefix | Meaning |
|--------|---------|
| `@@`   | Hunk separator. Optionally followed by a seek-anchor line (see below). |
| `-`    | Line to remove. Must be found in the file exactly. |
| `+`    | Line to add. Inserted in place of the removed lines. |
| ` `    | Context line (leading space). Present in both old and new; used to locate the edit region. |

Empty lines between hunks are silently ignored.

## Seek anchor (`@@ <text>`)

The text after `@@ ` is used as a single-line anchor. `seek_sequence` locates \
that line in the file first, then searches for `old_lines` starting from there.

```
@@ def calculate_total
-    total = 0
+    total = 0.0
```

No text after `@@` → search continues from the previous hunk's end position.

## EOF anchor

A line `-*** End of File` (case-insensitive) marks the hunk as end-of-file anchored. \
`seek_sequence` will prefer matching `old_lines` at the very end of the file.

## Output format

- Success:  `\"{path} ({N} hunk(s) applied)\"`
- Error:    `\"{path}\\n{error description}\"`

All results use `ToolContent::Text` with `is_error = false` — the model reads the inline text.

## Worked examples

### Single hunk

```
<edit path=\"/path/to/file.rs\">
<<<<
@@
-fn old_name() {
+fn new_name() {
>>>>
```

### Multi-hunk

```
<edit path=\"/path/to/file.rs\">
<<<<
@@
-import { oldFunc } from './lib';
+import { newFunc } from './lib';
@@
-oldFunc(x, y)
+newFunc(x, y)
>>>>
```

### @@ with seek context

```
<edit path=\"/path/to/file.py\">
<<<<
@@ def calculate_total
-    return sum(x)
+    return sum(item.price for item in items)
>>>>
```

### Context lines for disambiguation

```
<edit path=\"/path/to/file.rs\">
<<<<
@@
 fn process() {
-    let result = old_call();
+    let result = new_call();
 }
>>>>
```

### Pure deletion (empty + block)

```
<edit path=\"/path/to/file.rs\">
<<<<
@@
-// TODO: remove this comment
>>>>
```

## Error messages

- `hunk N: no match found` → `old_lines` not found in the file. Re-read and retry.
- `hunk N: seek context not found: <text>` → The anchor line after `@@` was not found.
- `hunk N and hunk M: overlapping matches` → Two hunks matched overlapping regions.
- `failed to read file: ...` → File does not exist or permission denied."
                .to_string(),
        },
    }
}

/// Parses `args_json` and executes the edit tool.
///
/// Returns a `ToolResult` with `ToolContent::Text` for both success and error.
/// Returns `Err(EditToolError::ArgsParse)` only if the `path` attr is missing
/// or the `__body__` diff body is absent or contains no valid hunks.
///
/// # Arguments
/// - `call_id`:   The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments injected by the dispatcher. Must contain
///                `"path"` (String) and `"__body__"` (String with diff body).
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, EditToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the edit tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, EditToolError> {
    // Parse arguments manually (no serde Deserialize).
    // A missing path or unparseable body is the only hard failure — all hunk
    // match errors are returned as Ok(ToolResult) with inline text.
    let args = EditArgs::parse(&args_json).map_err(EditToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "edit",
            Some(args.path.clone()),
            format!("Editing {} ({} hunk(s))", args.path, args.hunks.len()),
        ),
    );

    // Execute and return. The executor always returns Ok(ToolResult) — it never
    // panics or propagates an Err from file I/O (errors become inline text).
    Ok(executor::execute(call_id, args).await)
}
