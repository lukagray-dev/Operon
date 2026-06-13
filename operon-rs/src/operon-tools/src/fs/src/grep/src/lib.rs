//! # operon-tools-fs-grep
//!
//! Implements the `grep` tool for the Operon agent's filesystem group.
//!
//! Searches files and directories for regex patterns. Supports:
//! - Multiple OR-combined regex patterns (match if any pattern matches)
//! - Recursive directory walking with gitignore rules respected
//! - Filename glob filtering (e.g., "*.py" to search only Python files)
//! - Directory/entry ignore patterns
//! - Context lines before/after matches
//! - Per-file match reporting with line numbers
//! - 300 match limit to prevent context overflow
//! - 10 MB file size limit
//! - Binary file detection and skipping
//! - Glob-only mode (no patterns → lists matching files)
//!
//! ## Call format
//!
//! ```text
//! <grep path="C:\absolute\path\to\directory">
//!
//! <grep path="C:\absolute\path\to\directory">
//! <<<<
//! pattern="calculate_total"
//! glob="*.py"
//! ignore="node_modules" ".git"
//! context="3"
//! >>>>
//!
//! <!-- Glob-only: no pattern = lists matching files -->
//! <grep path="C:\absolute\path\to\directory">
//! <<<<
//! glob="*.py"
//! >>>>
//! ```

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::GrepArgs;
pub use error::GrepToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `grep` tool.
///
/// - `short`: sent to the model under normal conditions. Concise.
/// - `detailed`: sent after a malformed call. Full explanation with body format.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "grep".to_string(),
            description: "Search files for regex patterns. path attr is the root directory or file. \
                          Write search options in the tool body: pattern=\"regex\" for one or more \
                          patterns (space-separated quoted values), glob=\"*.py\" to filter file types, \
                          ignore=\"node_modules\" to skip directories, context=\"3\" for context lines. \
                          Omit pattern for glob-only file listing. Results capped at 300 matches."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "grep".to_string(),
            description: "\
Searches files and directories for regex patterns. Returns plain-text output with line numbers.

## Call format

The `path` attribute is the root directory or file to search. All options go in the tool body.

Simple search:
  <grep path=\"C:\\absolute\\path\\to\\directory\">
  <<<<
  pattern=\"calculate_total\"
  >>>>

Multiple OR patterns (line matches if ANY pattern matches):
  <grep path=\"C:\\absolute\\path\\to\\directory\">
  <<<<
  pattern=\"calculate_total\" \"Auth\" \"AuthManager\"
  glob=\"*.py\"
  ignore=\"node_modules\" \".git\"
  context=\"3\"
  >>>>

Glob-only mode (no pattern — lists matching files without searching content):
  <grep path=\"C:\\absolute\\path\\to\\directory\">
  <<<<
  glob=\"*.py\"
  >>>>

Simple (no body — searches all files with no filters):
  <grep path=\"C:\\absolute\\path\\to\\directory\">

## Body options

- `pattern`: One or more regex patterns (space-separated quoted values).
             Multiple patterns are OR-combined: a line matches if ANY pattern matches.
             Omit for glob-only mode.
- `glob`: Glob pattern to filter files during walk (e.g. \"*.rs\", \"*.{ts,tsx}\").
          Only the first token is used.
- `ignore`: Entry names to skip during walk. Each token is a separate pattern.
            Matched against file/dir names (not full paths).
- `context`: Number of context lines before and after each match. Default: 0.

## Output format

GLOB-ONLY MODE:
  {count} file(s) matched

  C:\\path\\to\\file1.py (4.2 KB)
  C:\\path\\to\\file2.py (9.3 KB)

SEARCH MODE:
  {total} match(es) in {files} file(s)

  C:\\path\\to\\file.py
  12| def helper():
  13|
  14| def calculate_total(items):

  C:\\path\\to\\file2.py
  88| def calculate_total(items):

  ***omitted 120 matches***

Files and match groups within a file are separated by blank lines.
Truncation notice appears at end if more than 300 matches were found.

File errors (binary, too large, permission denied):
  C:\\path\\to\\file.bin
  ERROR: binary file, skipped

## Constraints

- Maximum 300 total matches; excess matches are omitted with a notice.
- Files larger than 10 MB are skipped with an inline error.
- Binary files are skipped with an inline error.
- Regex uses Rust regex syntax. Special chars must be escaped for literal matching.

## Common mistakes

- Forgetting to escape regex special chars: use \"main\\\\(\\\\)\" not \"main()\".
- Expecting glob to filter direct file paths — glob only affects directory walks.
- Passing pattern as a JSON field instead of in the tool body."
                .to_string(),
        },
    }
}

/// Parses `args_json` and executes the grep tool.
///
/// Returns a `ToolResult` with `is_error: false` always — errors are embedded
/// in the plain-text output. Returns `Err(GrepToolError::ArgsParse)` only if
/// the `path` attribute is missing or the body is structurally invalid.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON arguments sent by the parser.
///
/// # Returns
/// - `Ok(ToolResult)` with search results as plain text.
/// - `Err(GrepToolError::ArgsParse)` if the arguments are malformed.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, GrepToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Parses `args_json` and executes the grep tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, GrepToolError> {
    // Parse the arguments from path attr + body.
    let args = GrepArgs::parse(&args_json).map_err(GrepToolError::ArgsParse)?;

    // Emit progress so the UI shows "Searching {path}" while waiting.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "grep",
            Some(args.path.clone()),
            format!("Searching {}", args.path),
        ),
    );

    // Execute the search and return the result (always Ok — errors are inline).
    Ok(executor::execute(call_id, args).await)
}
