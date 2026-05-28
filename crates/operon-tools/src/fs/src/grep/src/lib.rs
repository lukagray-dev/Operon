//! # operon-tools-fs-grep
//!
//! Implements the `grep` tool for the Operon agent's filesystem group.
//!
//! Searches files and directories for regex patterns. Supports:
//! - Regex pattern matching with case-sensitive/insensitive modes
//! - Recursive directory walking with gitignore rules respected
//! - Filename glob filtering (e.g., "*.rs" to search only Rust files)
//! - Context lines before/after matches
//! - Per-file match reporting with line numbers
//! - 300 match limit to prevent context overflow
//! - 10 MB file size limit
//! - Binary file detection and skipping
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_fs_grep::{definition, execute};
//! use operon_context_normalize_tools::ToolCallId;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let args = json!({
//!     "pattern": "fn main",
//!     "paths": ["src/"],
//!     "include": "*.rs"
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

pub use args::GrepArgs;
pub use error::GrepToolError;
pub use output::{FileGrepResult, GrepLine, GrepOutput};

use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::TieredToolDefinition;
use serde_json::json;

/// Returns the tiered tool definition for the `grep` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the key constraints (300 match limit, 10 MB file limit).
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   edge cases, common mistakes, and worked examples.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "Regex pattern to search for. Uses Rust regex syntax."
            },
            "paths": {
                "type": "array",
                "items": { "type": "string" },
                "minItems": 1,
                "description": "Files or directories to search. Directories are walked recursively."
            },
            "include": {
                "type": "string",
                "description": "Optional glob pattern to filter files by name (e.g., \"*.rs\", \"*.{ts,tsx}\")."
            },
            "case_insensitive": {
                "type": "boolean",
                "description": "Case-insensitive matching. Default: false."
            },
            "context_lines": {
                "type": "integer",
                "minimum": 0,
                "description": "Number of context lines before/after each match. Default: 0."
            }
        },
        "required": ["pattern", "paths"]
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "grep".to_string(),
            description: "Searches files and directories for a regex pattern. \
                          Pass `pattern` (regex string) and `paths` (array of file/dir paths). \
                          Directories are walked recursively respecting .gitignore. \
                          Use `include` to filter by filename glob (e.g., \"*.rs\"). \
                          Results capped at 300 matches. Files >10 MB are skipped."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "grep".to_string(),
            description: "\
Searches files and directories for a regex pattern. Returns structured per-file results with line numbers and context.

## Input shapes

Required parameters:
- `pattern` (string): Regex pattern using Rust regex syntax. Special characters must be escaped for literal matching.
- `paths` (array of strings): Files or directories to search. At least one path required.

Optional parameters:
- `include` (string): Glob pattern to filter filenames during directory walks (e.g., \"*.rs\", \"*.{ts,tsx}\").
- `case_insensitive` (boolean): Enable case-insensitive matching. Default: false.
- `context_lines` (integer): Number of lines to show before/after each match. Default: 0.

Example calls:
```json
{\"pattern\": \"TODO\", \"paths\": [\"src/\"]}
{\"pattern\": \"fn main\", \"paths\": [\"src/\"], \"include\": \"*.rs\"}
{\"pattern\": \"error\", \"paths\": [\"app.log\"], \"case_insensitive\": true, \"context_lines\": 2}
```

## Response format

Returns a JSON object:
```json
{
  \"total_matches\": 42,
  \"files_with_matches\": 5,
  \"truncated\": false,
  \"files\": [
    {
      \"path\": \"src/main.rs\",
      \"match_count\": 3,
      \"matches\": [
        {\"line_no\": 10, \"content\": \"fn main() {\", \"is_match\": true},
        {\"line_no\": 11, \"content\": \"    println!(\\\"Hello\\\");\", \"is_match\": false}
      ]
    }
  ]
}
```

Each file entry contains:
- `path`: The file that was searched
- `match_count`: Number of matching lines (excludes context lines)
- `matches`: Array of matching lines and context lines
  - `line_no`: 1-indexed line number in the source file
  - `content`: Line text with trailing newline removed
  - `is_match`: true for matching lines, false for context lines
- `error`: Present only if the file could not be searched (permission denied, binary file, too large, etc.)

## Behavior details

**Directory walking:**
- Directories in `paths` are walked recursively
- Gitignore rules are respected (files in .gitignore are skipped)
- Hidden files and directories are skipped by default
- The `include` glob filter is applied during the walk (only affects directories, not direct file paths)

**Pattern matching:**
- `pattern` is always treated as a regex, not a literal string
- Use Rust regex syntax: https://docs.rs/regex/latest/regex/#syntax
- To match literal special characters, escape them: `\\.` for dot, `\\(` for parenthesis, etc.
- Case-sensitive by default; use `case_insensitive: true` for case-insensitive matching

**Context lines:**
- Context lines from adjacent matches may overlap or merge
- Context lines are marked with `is_match: false`
- The same line will not appear twice in the output

**Limits and truncation:**
- Maximum 300 total matches across all files
- When the limit is hit, `truncated: true` is set in the response
- The current file being searched is finished, but subsequent files are skipped
- Files larger than 10 MB are skipped with an error message
- Binary files (containing null bytes) are skipped with an error message

**Output filtering:**
- Only files with matches or errors appear in the `files` array
- Files with zero matches and no errors are omitted to reduce output size

## Common mistakes

1. **Forgetting to escape regex special characters:**
   - Wrong: `{\"pattern\": \"main()\", \"paths\": [\"src/\"]}`  ← `()` are regex groups
   - Right: `{\"pattern\": \"main\\\\(\\\\)\", \"paths\": [\"src/\"]}`  ← escaped for literal match

2. **Expecting `include` to filter direct file paths:**
   - `include` only affects directory walks, not files listed directly in `paths`
   - If `paths: [\"main.py\", \"test.js\"]` and `include: \"*.py\"`, both files are still searched

3. **Passing `paths` as a string instead of an array:**
   - Wrong: `{\"pattern\": \"TODO\", \"paths\": \"src/\"}`
   - Right: `{\"pattern\": \"TODO\", \"paths\": [\"src/\"]}`

4. **Assuming `is_match: false` means no match:**
   - `is_match: false` indicates a context line, not a failed match
   - Context lines are included via the `context_lines` parameter

5. **Not checking the `truncated` field:**
   - If `truncated: true`, results are incomplete
   - Refine the search (more specific pattern, narrower paths, use `include` filter)

## Regex syntax quick reference

- `.` — any character (except newline)
- `*` — zero or more of the preceding
- `+` — one or more of the preceding
- `?` — zero or one of the preceding
- `^` — start of line
- `$` — end of line
- `\\b` — word boundary
- `[abc]` — any of a, b, or c
- `[^abc]` — any character except a, b, or c
- `(a|b)` — either a or b
- `\\d` — digit (0-9)
- `\\w` — word character (a-z, A-Z, 0-9, _)
- `\\s` — whitespace

Escape special characters with `\\` for literal matching: `\\.`, `\\*`, `\\(`, `\\)`, `\\[`, `\\]`, etc."
                .to_string(),
            parameters,
        },
    }
}

/// Deserializes `args_json` and executes the grep tool.
///
/// Returns a `ToolResult` with `is_error: false` for successful searches (even if no matches found).
/// Returns `is_error: true` only for invalid regex patterns or internal serialization bugs.
/// Per-file errors (permission denied, binary files, etc.) are embedded in the JSON content.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args_json`: The raw JSON arguments sent by the model.
///
/// # Returns
/// - `Ok(ToolResult)` with per-file results in JSON content.
/// - `Err(GrepToolError::ArgsParse)` if the arguments are malformed.
///
/// # Example
/// ```rust
/// # use operon_tools_fs_grep::execute;
/// # use operon_context_normalize_tools::ToolCallId;
/// # use serde_json::json;
/// # async fn example() {
/// let result = execute(
///     ToolCallId("call_123".to_string()),
///     json!({ "pattern": "TODO", "paths": ["src/"] })
/// ).await.unwrap();
/// assert_eq!(result.name, "grep");
/// # }
/// ```
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, GrepToolError> {
    // Deserialize the arguments. If this fails, return an ArgsParse error.
    let args: GrepArgs = serde_json::from_value(args_json)?;
    
    // Execute the tool and return the result. The executor always returns a
    // ToolResult (never panics), so we can directly return it.
    Ok(executor::execute(call_id, args).await)
}
