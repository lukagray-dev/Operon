//! Chunk parser for unified-diff style patch hunks in Operon's edit tool.
//!
//! Hey friend! This module is responsible for parsing a patch string (which contains
//! one or more `@@` hunk headers, context lines, additions, and deletions) into a vector
//! of structured `UpdateFileChunk` instances.
//!
//! It is ported from Codex's `streaming_parser.rs` (specifically the `UpdateFile` mode),
//! stripped down for single-file tool calls (no envelope markers like `*** Begin Patch`).

use thiserror::Error;

/// Error type encountered while parsing unified-diff patch chunks.
#[derive(Debug, PartialEq, Clone, Error)]
pub enum ChunkParseError {
    /// The input patch string was empty or contained only whitespace.
    #[error("patch string is empty")]
    EmptyPatch,

    /// No valid `@@` hunk header was found in the patch.
    #[error("invalid patch: patch must contain at least one @@ hunk header")]
    NoHunksFound,

    /// Syntax error on a specific line of the patch hunk.
    #[error("invalid patch hunk on line {line_number}: {message}")]
    InvalidHunk {
        /// Explanation of what went wrong.
        message: String,
        /// 1-based line number in the patch string where the error occurred.
        line_number: usize,
    },
}

/// A single update chunk parsed from a patch.
///
/// Contains an optional change context line (from `@@ <context>`) and vectors
/// of `old_lines` (lines to replace) and `new_lines` (replacement lines).
#[derive(Debug, PartialEq, Clone)]
pub struct UpdateFileChunk {
    /// A single line of context used to narrow down the position of the chunk
    /// (e.g., function or class definition).
    pub change_context: Option<String>,

    /// A contiguous block of lines that should be replaced with `new_lines`.
    pub old_lines: Vec<String>,

    /// The replacement lines for `old_lines`.
    pub new_lines: Vec<String>,

    /// Whether this chunk is anchored at the end of the file.
    pub is_end_of_file: bool,
}

/// Parse a raw patch string into a sequence of `UpdateFileChunk` structs.
///
/// # Format Specs
/// - A hunk begins with `@@` (bare context) or `@@ <context_text>`.
/// - Following lines inside a hunk must begin with:
///   - `' '` (space): Context line present in both old and new content.
///   - `'-'`: Line present in old content, to be removed.
///   - `'+'`: Line to be added in new content.
///   - `""` (bare empty line): Treated as an empty context line for LLM leniency.
///
/// # Arguments
/// - `patch`: The input unified-diff patch string sent by the model.
///
/// # Returns
/// - `Ok(Vec<UpdateFileChunk>)` if parsing succeeds and all hunks are valid.
/// - `Err(ChunkParseError)` if syntax errors or missing hunks are detected.
pub fn parse_patch_chunks(patch: &str) -> Result<Vec<UpdateFileChunk>, ChunkParseError> {
    if patch.trim().is_empty() {
        return Err(ChunkParseError::EmptyPatch);
    }

    let mut chunks: Vec<UpdateFileChunk> = Vec::new();
    let mut line_number = 0;

    for raw_line in patch.lines() {
        line_number += 1;

        // Strip trailing \r if present (handles Windows CRLF line endings cleanly).
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let trimmed_end = line.trim_end();

        // ── 1. Check for Hunk Header (`@@` or `@@ <context>`) ──────────────
        if trimmed_end == "@@" {
            // Check that the previous chunk (if any) was not empty before starting a new one.
            if let Some(last_chunk) = chunks.last() {
                if last_chunk.old_lines.is_empty() && last_chunk.new_lines.is_empty() {
                    return Err(ChunkParseError::InvalidHunk {
                        message: "Update hunk does not contain any lines".to_string(),
                        line_number: line_number - 1,
                    });
                }
            }
            chunks.push(UpdateFileChunk {
                change_context: None,
                old_lines: Vec::new(),
                new_lines: Vec::new(),
                is_end_of_file: false,
            });
            continue;
        }

        if let Some(context_text) = trimmed_end.strip_prefix("@@ ") {
            if let Some(last_chunk) = chunks.last() {
                if last_chunk.old_lines.is_empty() && last_chunk.new_lines.is_empty() {
                    return Err(ChunkParseError::InvalidHunk {
                        message: "Update hunk does not contain any lines".to_string(),
                        line_number: line_number - 1,
                    });
                }
            }
            chunks.push(UpdateFileChunk {
                change_context: Some(context_text.to_string()),
                old_lines: Vec::new(),
                new_lines: Vec::new(),
                is_end_of_file: false,
            });
            continue;
        }

        // ── 2. Handle Lines Before Any Header ──────────────────────────────
        if chunks.is_empty() {
            // Ignore leading blank lines before the first @@
            if line.trim().is_empty() {
                continue;
            }
            return Err(ChunkParseError::InvalidHunk {
                message: format!(
                    "Expected update hunk to start with a @@ context marker, got: '{line}'"
                ),
                line_number,
            });
        }

        // ── 3. Parse Hunk Line Content ─────────────────────────────────────
        let current_chunk = chunks.last_mut().expect("chunks is non-empty");

        if let Some(content) = line.strip_prefix(' ') {
            // Context line: present in both old and new versions.
            current_chunk.old_lines.push(content.to_string());
            current_chunk.new_lines.push(content.to_string());
        } else if let Some(content) = line.strip_prefix('+') {
            // Added line: present only in new version.
            current_chunk.new_lines.push(content.to_string());
        } else if let Some(content) = line.strip_prefix('-') {
            // Removed line: present only in old version.
            current_chunk.old_lines.push(content.to_string());
        } else if line.is_empty() {
            // Bare empty line: treat as empty context line for leniency.
            current_chunk.old_lines.push(String::new());
            current_chunk.new_lines.push(String::new());
        } else {
            return Err(ChunkParseError::InvalidHunk {
                message: format!(
                    "Unexpected line found in update hunk: '{line}'. Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)"
                ),
                line_number,
            });
        }
    }

    // ── 4. Final Validation ────────────────────────────────────────────────
    if chunks.is_empty() {
        return Err(ChunkParseError::NoHunksFound);
    }

    if let Some(last_chunk) = chunks.last() {
        if last_chunk.old_lines.is_empty() && last_chunk.new_lines.is_empty() {
            return Err(ChunkParseError::InvalidHunk {
                message: "Update hunk does not contain any lines".to_string(),
                line_number,
            });
        }
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_hunk() {
        let patch = "@@ fn hello()\n-old_line\n+new_line\n context";
        let chunks = parse_patch_chunks(patch).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].change_context, Some("fn hello()".to_string()));
        assert_eq!(chunks[0].old_lines, vec!["old_line", "context"]);
        assert_eq!(chunks[0].new_lines, vec!["new_line", "context"]);
    }

    #[test]
    fn test_parse_multiple_hunks() {
        let patch = "\
@@ fn first()
-a
+b
@@ fn second()
-c
+d
";
        let chunks = parse_patch_chunks(patch).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].change_context, Some("fn first()".to_string()));
        assert_eq!(chunks[0].old_lines, vec!["a"]);
        assert_eq!(chunks[0].new_lines, vec!["b"]);
        assert_eq!(chunks[1].change_context, Some("fn second()".to_string()));
        assert_eq!(chunks[1].old_lines, vec!["c"]);
        assert_eq!(chunks[1].new_lines, vec!["d"]);
    }

    #[test]
    fn test_bare_context_header() {
        let patch = "@@\n-foo\n+bar\n";
        let chunks = parse_patch_chunks(patch).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].change_context, None);
        assert_eq!(chunks[0].old_lines, vec!["foo"]);
        assert_eq!(chunks[0].new_lines, vec!["bar"]);
    }

    #[test]
    fn test_empty_line_treated_as_blank_context() {
        let patch = "@@\n context before\n\n context after";
        let chunks = parse_patch_chunks(patch).unwrap();
        assert_eq!(chunks[0].old_lines, vec!["context before", "", "context after"]);
        assert_eq!(chunks[0].new_lines, vec!["context before", "", "context after"]);
    }

    #[test]
    fn test_missing_header_fails() {
        let patch = "-foo\n+bar";
        let result = parse_patch_chunks(patch);
        assert!(matches!(result, Err(ChunkParseError::InvalidHunk { .. })));
    }
}
