/// Executor for the read tool — handles all file I/O and result assembly.
///
/// This module contains the core logic for reading files, applying line ranges,
/// detecting binary content, enforcing size limits, and assembling the final
/// ToolResult. All file I/O is async via tokio::fs.
///
/// # Output format (plain text, not JSON)
///
/// Every content line is prefixed with its 1-indexed absolute line number:
///   "12| def helper():\n"
///   "13|\n"              (empty line — no trailing space after pipe)
///
/// All reads (single-file or multi-file) always include a path header:
///   Success, full read:   "{path}\n{numbered content}"
///   Success, range read:  "{path} lines N-M of Total\n{numbered content}"
///   Failure:              "{path}\nERROR: reason"
///
/// Multiple files are joined with "\n\n" between entries.
use crate::args::{ReadArgs, ReadTarget};
use crate::output::{FileReadResult, LineRange};
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Maximum file size for full-file reads (1 MB).
///
/// Files larger than this limit cannot be read in a single call without
/// specifying a line range. This prevents accidentally loading huge files
/// into memory and overwhelming the model's context window.
const MAX_FILE_SIZE_BYTES: usize = 1_048_576;

/// Executes the read tool with the given arguments.
///
/// Reads all requested files concurrently (bounded by semaphore), applies line
/// ranges if specified, detects binary content, enforces size limits, and
/// assembles a plain-text ToolResult with line-numbered output.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args`: The parsed read arguments containing the list of targets.
///
/// # Returns
/// A `ToolResult` with `is_error: false` always. Per-file errors are embedded
/// inline in the text output, not surfaced as a top-level error.
pub async fn execute(call_id: ToolCallId, args: ReadArgs) -> ToolResult {
    // Maximum number of concurrent file reads to prevent file descriptor exhaustion.
    const MAX_CONCURRENT_READS: usize = 16;
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_READS));

    // Read all files concurrently using join_all, bounded by the semaphore.
    let futures: Vec<_> = args
        .targets
        .into_iter()
        .map(|target| {
            let sem = Arc::clone(&semaphore);
            async move {
                // Acquire the semaphore permit before reading to limit concurrency.
                let _permit = sem.acquire().await.expect("semaphore closed");
                read_single_file(target).await
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    // Collect paths that were successfully read for the read ledger.
    let successful_paths: Vec<String> = results
        .iter()
        .filter(|r| r.success)
        .map(|r| r.path.clone())
        .collect();

    // Format each file result into a text segment with line-numbered output.
    // The _is_multi parameter is no longer meaningful — path headers always shown.
    let segments: Vec<String> = results
        .into_iter()
        .map(|r| format_file_result(r, true))
        .collect();

    // Join multiple file segments with a blank line between them.
    let output_text = if segments.len() == 1 {
        segments.into_iter().next().unwrap_or_default()
    } else {
        segments.join("\n\n")
    };

    // Build the final ToolResult — is_error is always false.
    // Per-file errors are embedded in the text output.
    ToolResult {
        call_id,
        name: "read".to_string(),
        content: ToolContent::Text(output_text),
        is_error: false,
        // Populate read_paths so the dispatcher can update the read ledger
        // without parsing the text output.
        read_paths: Some(successful_paths),
    }
}

/// Format a FileReadResult into a line-numbered text segment for the tool output.
///
/// All output cases include a path header. Every content line is prefixed with
/// its 1-indexed absolute line number in the format "{line_no}| {content}\n".
///
/// Formats:
///   Success, full read:   "{path}\n{numbered content}"
///   Success, range read:  "{path} lines N-M of Total\n{numbered content}"
///   Failure:              "{path}\nERROR: reason"
///
/// The `_is_multi` parameter is kept for API compatibility but no longer
/// changes the output — path headers are always shown.
fn format_file_result(result: FileReadResult, _is_multi: bool) -> String {
    if !result.success {
        // Error case: always show the path plus the error message.
        return format!(
            "{}\nERROR: {}",
            result.path,
            result.error.as_deref().unwrap_or("unknown error")
        );
    }

    let content = result.content.as_deref().unwrap_or("");

    // Determine the starting line number for numbering.
    // For range reads, line numbering starts at the first line of the range.
    // For full reads, numbering starts at 1.
    let start_line_no = result
        .lines_returned
        .as_ref()
        .map(|r| r.start)
        .unwrap_or(1);

    // Prefix each line with its 1-indexed absolute line number.
    // Empty lines get "13|\n" (no trailing space after the pipe).
    let numbered: String = content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{}| {}\n", start_line_no + i, line))
        .collect();

    if let Some(range) = result.lines_returned {
        // Range read: include path header with range info.
        let total = result.total_lines.unwrap_or(0);
        format!(
            "{} lines {}-{} of {}\n{}",
            result.path, range.start, range.end, total, numbered
        )
    } else {
        // Full read: include path header (no special single-file treatment).
        format!("{}\n{}", result.path, numbered)
    }
}

/// Reads a single file and returns a FileReadResult.
///
/// This function handles all the logic for a single file read:
/// - Size checking (for full-file reads, 1 MB limit)
/// - Binary detection (null bytes → error)
/// - UTF-8 validation
/// - CRLF normalization (\r\n → \n and standalone \r → \n)
/// - Line range application (if start_line or end_line is specified)
///
/// # Arguments
/// - `target`: The read target containing the path and optional line range.
///
/// # Returns
/// A `FileReadResult` with either success + content or failure + error message.
async fn read_single_file(target: ReadTarget) -> FileReadResult {
    let path_str = target.path.clone();
    let path = Path::new(&path_str);

    // Determine if this is a line-range read or a full-file read.
    let is_range_read = target.start_line.is_some() || target.end_line.is_some();

    // For full-file reads, check the size first to avoid loading huge files.
    // Range reads bypass the size check — they read the full file and then slice.
    if !is_range_read {
        match tokio::fs::metadata(path).await {
            Ok(metadata) => {
                let size = metadata.len() as usize;
                if size > MAX_FILE_SIZE_BYTES {
                    // File is too large for a full read. Return an error result.
                    return FileReadResult {
                        path: path_str,
                        success: false,
                        content: None,
                        error: Some(format!(
                            "File exceeds 1 MB limit ({} bytes). Use a line range to read in chunks.",
                            size
                        )),
                        total_lines: None,
                        lines_returned: None,
                    };
                }
            }
            Err(e) => {
                // Failed to get metadata (file doesn't exist, permission denied, etc.).
                return FileReadResult {
                    path: path_str,
                    success: false,
                    content: None,
                    error: Some(format!("Failed to access file: {}", e)),
                    total_lines: None,
                    lines_returned: None,
                };
            }
        }
    }

    // Read the file contents as raw bytes.
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) => {
            // Failed to read the file (doesn't exist, permission denied, etc.).
            return FileReadResult {
                path: path_str,
                success: false,
                content: None,
                error: Some(format!("Failed to read file: {}", e)),
                total_lines: None,
                lines_returned: None,
            };
        }
    };

    // Binary detection: check for null bytes in the file content.
    // Null bytes indicate a non-text (binary) file — we cannot show these to the model.
    if bytes.contains(&0) {
        return FileReadResult {
            path: path_str,
            success: false,
            content: None,
            error: Some(
                "Binary file detected. Use the image/video tool for media files.".to_string(),
            ),
            total_lines: None,
            lines_returned: None,
        };
    }

    // Convert bytes to a UTF-8 string. Use strict conversion to detect invalid UTF-8.
    let raw_content = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            return FileReadResult {
                path: path_str,
                success: false,
                content: None,
                error: Some(
                    "File contains invalid UTF-8 encoding. \
                     May be a binary or non-UTF-8 encoded text file (e.g. Windows-1252, Latin-1). \
                     Only UTF-8 encoded text files are supported."
                        .to_string(),
                ),
                total_lines: None,
                lines_returned: None,
            };
        }
    };

    // CRLF normalization: replace \r\n → \n first (order matters!), then standalone \r → \n.
    // This ensures consistent line endings regardless of the file's original line-ending style.
    let full_content = raw_content.replace("\r\n", "\n").replace('\r', "\n");

    // If this is a line-range read, apply the range and return.
    if is_range_read {
        return apply_line_range(path_str, full_content, target.start_line, target.end_line);
    }

    // Full-file read: count total lines and return the entire content.
    let total_lines = count_lines(&full_content);

    FileReadResult {
        path: path_str,
        success: true,
        content: Some(full_content),
        error: None,
        total_lines: Some(total_lines),
        lines_returned: None,
    }
}

/// Applies a line range to the full file content and returns a FileReadResult.
///
/// This function splits the content into lines, applies the requested range
/// (clamping to the actual line count), and assembles the result string.
///
/// # Arguments
/// - `path`: The file path (included in the result for output formatting).
/// - `content`: The full file content as a string (already CRLF-normalized).
/// - `start_line`: Optional start line (1-indexed, inclusive). None = line 1.
/// - `end_line`: Optional end line (1-indexed, inclusive). None = EOF.
///
/// # Returns
/// A `FileReadResult` with the requested line range or an error if start_line
/// is beyond the end of the file.
fn apply_line_range(
    path: String,
    content: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> FileReadResult {
    // Split the content into lines. Since we normalized CRLF, we only have \n.
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Default start_line to 1 if not specified.
    let start = start_line.unwrap_or(1);

    // Default end_line to total_lines if not specified.
    let end = end_line.unwrap_or(total_lines);

    // Validate that start_line is within bounds.
    if start > total_lines {
        return FileReadResult {
            path,
            success: false,
            content: None,
            error: Some(format!(
                "start_line {} exceeds file length ({} lines).",
                start, total_lines
            )),
            total_lines: Some(total_lines),
            lines_returned: None,
        };
    }

    // Clamp end_line to total_lines (don't error if end > total_lines, just clamp).
    let end_clamped = end.min(total_lines);

    // Convert 1-indexed to 0-indexed for slicing.
    let start_idx = start.saturating_sub(1);
    let end_idx = end_clamped; // end is inclusive so we don't subtract 1 (slice excludes end_idx)

    // Extract the requested lines.
    let selected_lines = &lines[start_idx..end_idx];

    // Check if the original content had a trailing newline — we need to preserve
    // it when we are at the end of file and the file originally ended with one.
    let has_trailing_newline = content.ends_with('\n');

    // Reconstruct the content with proper line endings.
    let mut result_content = String::new();
    for (i, line) in selected_lines.iter().enumerate() {
        result_content.push_str(line);

        // Add a newline after each line except potentially the last.
        let is_last_in_selection = i == selected_lines.len() - 1;
        let is_at_eof = end_clamped == total_lines;

        if !is_last_in_selection {
            // Always add newline between lines in the selection.
            result_content.push('\n');
        } else if is_at_eof && has_trailing_newline {
            // At EOF with trailing newline: preserve the trailing newline.
            result_content.push('\n');
        } else if !is_at_eof {
            // Not at EOF: add newline (there are more lines after this selection).
            result_content.push('\n');
        }
        // else: at EOF without trailing newline — don't add one.
    }

    FileReadResult {
        path,
        success: true,
        content: Some(result_content),
        error: None,
        total_lines: Some(total_lines),
        lines_returned: Some(LineRange {
            start,
            end: end_clamped,
        }),
    }
}

/// Counts the number of lines in a string.
///
/// A line is defined as a sequence of characters ending with a newline, or the
/// final sequence of characters if the file doesn't end with a newline.
///
/// # Arguments
/// - `content`: The string to count lines in (should be CRLF-normalized to \n only).
///
/// # Returns
/// The number of lines in the string. 0 for an empty string.
fn count_lines(content: &str) -> usize {
    if content.is_empty() {
        return 0;
    }

    // Count newlines. If the file ends with a newline, the newline count equals
    // the line count. Otherwise, add 1 for the final unterminated line.
    let newline_count = content.matches('\n').count();
    if content.ends_with('\n') {
        newline_count
    } else {
        newline_count + 1
    }
}
