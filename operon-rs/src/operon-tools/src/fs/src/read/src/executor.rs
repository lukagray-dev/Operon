/// Executor for the read tool — handles all file I/O and result assembly.
///
/// This module contains the core logic for reading files, applying line ranges,
/// detecting binary content, enforcing size limits, and assembling the final
/// ToolResult. All file I/O is async via tokio::fs.
use crate::args::{ReadArgs, ReadTarget};
use crate::output::{FileReadResult, LineRange, ReadOutput};
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
/// Reads all requested files concurrently, applies line ranges if specified,
/// detects binary content, enforces size limits, and assembles a structured
/// ToolResult with per-file success/error information.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized read arguments containing the list of files to read.
///
/// # Returns
/// A `ToolResult` with `is_error: false` (even if individual files failed).
/// Per-file errors are embedded in the JSON content, not surfaced as a top-level error.
pub async fn execute(call_id: ToolCallId, args: ReadArgs) -> ToolResult {
    // Maximum number of concurrent file reads to prevent file descriptor exhaustion.
    const MAX_CONCURRENT_READS: usize = 16;
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_READS));

    let targets = args.into_targets();
    if targets.is_empty() {
        return ToolResult {
            call_id,
            name: "read".to_string(),
            content: ToolContent::Text("Error: No file paths provided to read.".to_string()),
            is_error: true,
        };
    }

    // Read all files concurrently using join_all, bounded by the semaphore.
    let futures: Vec<_> = targets
        .into_iter()
        .map(|target| {
            let sem = Arc::clone(&semaphore);
            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                read_single_file(target).await
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    // Assemble the final output structure and convert to plain text.
    let output = ReadOutput { files: results };

    // Return the successful ToolResult with plain text content.
    ToolResult {
        call_id,
        name: "read".to_string(),
        content: ToolContent::Text(output.to_plain_text()),
        is_error: false,
    }
}

/// Reads a single file and returns a FileReadResult.
///
/// This function handles all the logic for a single file read:
/// - Size checking (for full-file reads)
/// - Binary detection
/// - Line range application (if specified)
/// - Error handling and result assembly
///
/// # Arguments
/// - `target`: The read target containing the path and optional line range.
///
/// # Returns
/// A `FileReadResult` with either success + content or failure + error message.
async fn read_single_file(target: ReadTarget) -> FileReadResult {
    let path_str = target.path.clone();
    let path = Path::new(&path_str);

    if !path.is_absolute() {
        return FileReadResult {
            path: path_str,
            success: false,
            content: None,
            error: Some(
                "Path must be an absolute path. Relative paths are not supported.".to_string(),
            ),
            total_lines: None,
            lines_returned: None,
        };
    }

    // Determine if this is a line-range read or a full-file read.

    let is_range_read = target.start_line.is_some() || target.end_line.is_some();

    // For full-file reads, check the size first to avoid loading huge files.
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
                            "File exceeds 1 MB limit ({} bytes). Use start_line/end_line to read in chunks.",
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

    // Binary detection: check for null bytes.
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
    let full_content = match String::from_utf8(bytes) {
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

    // If this is a line-range read, apply the range.
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
/// (clamping to the actual line count), and assembles the result.
///
/// # Arguments
/// - `path`: The file path (for the result).
/// - `content`: The full file content as a string.
/// - `start_line`: Optional start line (1-indexed, inclusive).
/// - `end_line`: Optional end line (1-indexed, inclusive).
///
/// # Returns
/// A `FileReadResult` with the requested line range or an error if the range is invalid.
fn apply_line_range(
    path: String,
    content: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> FileReadResult {
    // Split the content into lines, preserving line endings.
    // We need to handle both \n and \r\n line endings.
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
    let end_idx = end_clamped; // end is inclusive, so we don't subtract 1 here.

    // Extract the requested lines and join them back into a string.
    let selected_lines = &lines[start_idx..end_idx];

    // Reconstruct the content with proper line endings.
    // We need to check if the original file had trailing newlines.
    let has_trailing_newline = content.ends_with('\n');

    let mut result_content = String::new();
    for (i, line) in selected_lines.iter().enumerate() {
        result_content.push_str(line);
        // Add newline after each line, including the last one if:
        // - It's not the last line in our selection, OR
        // - It IS the last line AND (we're at EOF with trailing newline OR we're not at EOF)
        let is_last_in_selection = i == selected_lines.len() - 1;
        let is_at_eof = end_clamped == total_lines;

        if !is_last_in_selection {
            // Always add newline between lines
            result_content.push('\n');
        } else if is_at_eof && has_trailing_newline {
            // At EOF with trailing newline: preserve it
            result_content.push('\n');
        } else if !is_at_eof {
            // Not at EOF: add newline (there are more lines after this)
            result_content.push('\n');
        }
        // else: at EOF without trailing newline, don't add one
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
/// - `content`: The string to count lines in.
///
/// # Returns
/// The number of lines in the string.
fn count_lines(content: &str) -> usize {
    if content.is_empty() {
        return 0;
    }

    // Count newlines and add 1 if the file doesn't end with a newline.
    let newline_count = content.matches('\n').count();
    if content.ends_with('\n') {
        newline_count
    } else {
        newline_count + 1
    }
}
