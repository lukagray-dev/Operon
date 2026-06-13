//! # operon-tools-fs-read
//!
//! Implements the `read` tool for the Operon agent's filesystem group.
//!
//! Reads one or multiple files in a single tool call. Supports:
//! - Multi-file batched reads (concurrent, up to 16 at a time)
//! - Chunked reading via line range syntax (e.g. `file.txt:40-90`, `file.txt:50-`, `file.txt:-30`)
//! - Binary file detection (null bytes → error)
//! - 1 MB size limit on full-file reads (range reads bypass it)
//! - CRLF normalization (\r\n → \n, standalone \r → \n)
//! - Per-file error inline in the text output (never a top-level is_error: true)
//! - Line-numbered output for every content line (e.g. "42| def foo():")
//!
//! ## Argument format
//!
//! The `paths` attribute is a single string containing a whitespace-separated list
//! of path entries. Each quoted value in the original call is joined with a space
//! by the dispatcher before arriving here. Each token is one path entry:
//!
//! ```text
//! C:\file.txt                   → full file read
//! C:\file.txt:40-90             → lines 40 to 90 inclusive (1-indexed)
//! C:\file.txt:50-               → line 50 to EOF
//! C:\file.txt:-30               → line 1 to line 30
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use operon_tools_fs_read::{definition, execute};
//! use operon_context_normalize::tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "paths": r"C:\src\main.rs C:\Cargo.toml"
//! });
//! let result = execute(
//!     ToolCallId("call_123".to_string()),
//!     args
//! ).await.unwrap();
//! # }
//! ```

mod args;
mod error;
mod executor;
mod output;

#[cfg(test)]
mod tests;

pub use args::{ReadArgs, ReadTarget};
pub use error::ReadToolError;
// FileReadResult and LineRange are internal — no longer re-exported.
// (ReadOutput has been removed; the read tool now outputs plain text.)

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `read` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the single most important constraint (1 MB limit, range syntax).
/// - `detailed`: sent after a malformed call. Full explanation with the paths attr
///   format, range syntax rules, output format, and error behavior.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "read".to_string(),
            description: "Read one or more files. paths is a space-separated list where each \
                          entry is a file path with an optional line range (e.g. C:\\file.txt:40-90, \
                          C:\\file.txt:50- for 50→EOF, C:\\file.txt:-30 for 1→30). Max 1 MB per \
                          full-file read. Binary files not supported. Output includes line numbers."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "read".to_string(),
            description: "\
Reads one or more files in a single call. Returns plain text output with line numbers.

## Input format

`paths` is a required string attribute containing a whitespace-separated list of path entries.
Multiple quoted values in the call are joined with spaces by the dispatcher.
Each token is ONE of:

1. A plain file path — reads the entire file (subject to 1 MB limit):
   `C:\\src\\main.rs`

2. A path with a line range — reads a subset of lines (bypasses 1 MB limit):
   `C:\\src\\main.rs:40-90`   ← lines 40 to 90 inclusive (1-indexed)
   `C:\\src\\main.rs:50-`     ← line 50 to EOF
   `C:\\src\\main.rs:-30`     ← line 1 to line 30

You can read multiple files in one call using space-separated quoted values:
  <read paths=\"C:\\Cargo.toml\" \"C:\\src\\main.rs:1-50\">

## Range colon detection rule

The range colon is the LAST colon where what follows matches `\\d*-\\d*`.
Drive-letter colons (e.g. `C:\\`) are at index 1 followed by a backslash — they never match.

## Size limit

Full-file reads (no range) are capped at 1 MB. If a file exceeds this, the output for
that file contains ERROR: File exceeds 1 MB limit. Use a line range to read in chunks.
Range reads bypass the size check entirely.

## Output format

Every content line is prefixed with its 1-indexed absolute line number:
  42| def calculate_total(items):
  43|     total = 0
  44|

All reads include a path header:
  - Success, full read:   \"{path}\\n{numbered content}\"
  - Success, range read:  \"{path} lines N-M of Total\\n{numbered content}\"
  - Failure:              \"{path}\\nERROR: reason\"

Multiple files (entries joined by a blank line):
  C:\\src\\main.rs
  1| fn main() {
  2|     println!(\"Hello\");
  3| }

  C:\\Cargo.toml
  1| [package]
  2| name = \"my-crate\"

The overall tool call always returns is_error: false. Per-file failures appear inline.

## Constraints

- Binary files (null bytes) → ERROR: Binary file detected.
- Invalid UTF-8 → ERROR: File contains invalid UTF-8 encoding.
- Non-existent path → ERROR: Failed to access file / Failed to read file.
- start_line beyond file end → ERROR: start_line N exceeds file length.
- end_line beyond file end → clamped silently to last line, no error.

## Common mistakes

- Passing `paths` as a JSON array instead of a string → args parse failure.
- Using `path` (singular) as the attr name instead of `paths` → args parse failure.
- Forgetting that multiple paths use space-separated quoted values, not semicolons."
                .to_string(),
        },
    }
}

/// Parses `args_json` and executes the read tool.
///
/// Returns a `ToolResult` with `is_error: false` even on partial file failures —
/// per-file errors are embedded inline in the text content.
/// Returns `Err(ReadToolError::ArgsParse)` only if the top-level argument shape is invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments sent by the parser (all values are strings).
///
/// # Returns
/// - `Ok(ToolResult)` with per-file results in plain text content.
/// - `Err(ReadToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, ReadToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the read tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, ReadToolError> {
    // Parse the arguments manually — no serde deserialization.
    // ReadArgs::parse returns Err(String) on any parse failure.
    let args = match ReadArgs::parse(&args_json) {
        Ok(a) => a,
        Err(reason) => return Err(ReadToolError::ArgsParse(reason)),
    };

    // Emit a progress event so the UI can show "Reading N file(s)" while waiting.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "read",
            None,
            format!("Reading {} file(s)", args.targets.len()),
        ),
    );

    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics or returns an error internally), so we wrap in Ok.
    Ok(executor::execute(call_id, args).await)
}
