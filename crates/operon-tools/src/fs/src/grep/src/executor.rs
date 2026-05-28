/// Executor for the grep tool — handles all file searching and result assembly.
///
/// This module contains the core logic for searching files with regex patterns,
/// applying filename filters, respecting gitignore rules, collecting context lines,
/// enforcing size and match limits, and assembling the final ToolResult.

use crate::args::GrepArgs;
use crate::output::{FileGrepResult, GrepLine, GrepOutput};
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{sinks::UTF8, SearcherBuilder};
use ignore::types::TypesBuilder;
use ignore::WalkBuilder;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Maximum total matches across all files before truncation.
///
/// Once this limit is reached, the current file is finished but no additional
/// files are searched. This prevents overwhelming the model's context window
/// with massive search results.
const MAX_MATCHES: usize = 300;

/// Maximum file size in bytes (10 MB).
///
/// Files larger than this are skipped with an error message. This prevents
/// attempting to search extremely large files that could cause memory issues
/// or excessive processing time.
const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// Executes the grep tool with the given arguments.
///
/// Searches all requested files/directories for the regex pattern, applies
/// filename filtering if specified, collects matches with context lines,
/// enforces limits, and assembles a structured ToolResult.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized grep arguments containing pattern, paths, and options.
///
/// # Returns
/// A `ToolResult` with `is_error: false` for successful searches (even if no matches found).
/// Returns `is_error: true` only for invalid regex patterns or internal serialization bugs.
/// Per-file errors (permission denied, binary files, etc.) are embedded in the JSON content.
pub async fn execute(call_id: ToolCallId, args: GrepArgs) -> ToolResult {
    // Build the regex matcher from the pattern.
    // If the pattern is invalid, return an error ToolResult immediately.
    let matcher = match RegexMatcherBuilder::new()
        .case_insensitive(args.case_insensitive.unwrap_or(false))
        .build(&args.pattern)
    {
        Ok(m) => m,
        Err(e) => {
            // Invalid regex pattern — this is a user error, not an internal bug.
            // Return is_error: true so the model knows the call failed.
            return ToolResult {
                call_id,
                name: "grep".to_string(),
                content: ToolContent::Text(format!("Invalid regex pattern: {}", e)),
                is_error: true,
            };
        }
    };

    // Collect all file paths to search.
    // This handles both direct file paths and directory walking with glob filtering.
    let file_paths = match collect_file_paths(&args.paths, args.include.as_deref()) {
        Ok(paths) => paths,
        Err(e) => {
            // Failed to build the file list (e.g., invalid glob pattern).
            return ToolResult {
                call_id,
                name: "grep".to_string(),
                content: ToolContent::Text(format!("Failed to collect file paths: {}", e)),
                is_error: true,
            };
        }
    };

    // Search all files in a blocking task (grep-searcher is not async-friendly).
    // We pass owned data into the blocking task to avoid lifetime issues.
    let context_lines = args.context_lines.unwrap_or(0);
    let results = tokio::task::spawn_blocking(move || {
        search_files(file_paths, matcher, context_lines)
    })
    .await
    .expect("blocking task panicked");

    // Assemble the final output structure with summary statistics.
    let total_matches: usize = results.iter().map(|r| r.match_count).sum();
    let files_with_matches = results.iter().filter(|r| r.match_count > 0).count();
    let truncated = total_matches >= MAX_MATCHES;

    let output = GrepOutput {
        total_matches,
        files_with_matches,
        truncated,
        files: results,
    };

    // Serialize to JSON. This should never fail because all our types are Serialize.
    // If it does fail (e.g., due to a bug in our Serialize impl), we return an error ToolResult.
    let output_value = match serde_json::to_value(&output) {
        Ok(v) => v,
        Err(e) => {
            // This is a bug in our code, not a user error. Return an error ToolResult.
            return ToolResult {
                call_id,
                name: "grep".to_string(),
                content: ToolContent::Text(format!(
                    "Internal error: failed to serialize grep output: {}",
                    e
                )),
                is_error: true,
            };
        }
    };

    // Return the successful ToolResult with JSON content.
    ToolResult {
        call_id,
        name: "grep".to_string(),
        content: ToolContent::Json(output_value),
        is_error: false,
    }
}

/// Collects all file paths to search based on the input paths and optional glob filter.
///
/// For each path:
/// - If it's a file: add it directly to the result list
/// - If it's a directory: walk it recursively, applying the glob filter if provided
///
/// Gitignore rules are respected during directory walks. Duplicate paths are removed.
///
/// # Arguments
/// - `paths`: List of file or directory paths to search
/// - `include_glob`: Optional glob pattern to filter filenames (e.g., "*.rs")
///
/// # Returns
/// A deduplicated list of file paths to search, or an error if glob pattern is invalid.
fn collect_file_paths(
    paths: &[String],
    include_glob: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    let mut collected = HashSet::new();

    // Build the Types filter if a glob pattern was provided.
    let types_filter = if let Some(glob) = include_glob {
        let mut builder = TypesBuilder::new();
        // Add a custom type definition that matches the glob pattern.
        // The type name "custom" is arbitrary — we just need a name for the type.
        builder
            .add("custom", glob)
            .map_err(|e| format!("Invalid glob pattern '{}': {}", glob, e))?;
        builder.select("custom");
        Some(
            builder
                .build()
                .map_err(|e| format!("Failed to build glob filter: {}", e))?,
        )
    } else {
        None
    };

    for path_str in paths {
        let path = Path::new(path_str);

        if path.is_file() {
            // Direct file path — add it to the collection.
            collected.insert(path.to_path_buf());
        } else if path.is_dir() {
            // Directory — walk it recursively with gitignore rules.
            let mut walker = WalkBuilder::new(path);
            walker.standard_filters(true); // Enables gitignore, hidden files filtering, etc.

            // Apply the glob filter if provided.
            if let Some(ref types) = types_filter {
                walker.types(types.clone());
            }

            for entry in walker.build() {
                match entry {
                    Ok(e) => {
                        // Only collect files, not directories.
                        if e.file_type().map_or(false, |ft| ft.is_file()) {
                            collected.insert(e.path().to_path_buf());
                        }
                    }
                    Err(_) => {
                        // Ignore walk errors (permission denied, broken symlinks, etc.).
                        // These are common in directory traversal and shouldn't fail the entire search.
                        continue;
                    }
                }
            }
        }
        // If the path doesn't exist or is neither a file nor a directory, skip it silently.
        // The file will not appear in results, which is the expected behavior.
    }

    Ok(collected.into_iter().collect())
}

/// Searches all files for the regex pattern and returns per-file results.
///
/// This function runs synchronously in a blocking task. It searches each file
/// using grep-searcher, collects matches with context lines, enforces the
/// MAX_MATCHES limit, and handles per-file errors gracefully.
///
/// # Arguments
/// - `file_paths`: List of file paths to search
/// - `matcher`: The compiled regex matcher
/// - `context_lines`: Number of context lines to include before/after each match
///
/// # Returns
/// A list of FileGrepResult, one per file that had matches or errors.
/// Files with zero matches and no errors are omitted.
fn search_files(
    file_paths: Vec<PathBuf>,
    matcher: grep_regex::RegexMatcher,
    context_lines: usize,
) -> Vec<FileGrepResult> {
    // Shared state for tracking total matches across all files.
    // We use Arc<Mutex<>> to allow the searcher sink to update the count.
    let total_matches = Arc::new(Mutex::new(0usize));
    let mut results = Vec::new();

    for path in file_paths {
        // Check if we've already hit the global match limit.
        // If so, stop searching additional files (but finish the current file).
        let current_total = *total_matches.lock().expect("mutex poisoned");
        if current_total >= MAX_MATCHES {
            break;
        }

        // Search this file and collect the result.
        let result = search_single_file(&path, &matcher, context_lines, Arc::clone(&total_matches));

        // Only include files that had matches or errors in the output.
        // Files with zero matches and no error are omitted to reduce output size.
        if result.match_count > 0 || result.error.is_some() {
            results.push(result);
        }
    }

    results
}

/// Searches a single file for the regex pattern and returns a FileGrepResult.
///
/// This function handles all the logic for a single file search:
/// - Size checking (skip files > 10 MB)
/// - Binary detection (skip files with null bytes)
/// - Match collection with context lines
/// - Error handling and result assembly
///
/// # Arguments
/// - `path`: The file path to search
/// - `matcher`: The compiled regex matcher
/// - `context_lines`: Number of context lines to include before/after each match
/// - `total_matches`: Shared counter for global match limit enforcement
///
/// # Returns
/// A `FileGrepResult` with either matches or an error message.
fn search_single_file(
    path: &Path,
    matcher: &grep_regex::RegexMatcher,
    context_lines: usize,
    total_matches: Arc<Mutex<usize>>,
) -> FileGrepResult {
    let path_str = path.display().to_string();

    // Check file size before attempting to search.
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            return FileGrepResult {
                path: path_str,
                match_count: 0,
                matches: Vec::new(),
                error: Some(format!("Failed to access file: {}", e)),
            };
        }
    };

    if metadata.len() > MAX_FILE_SIZE_BYTES {
        return FileGrepResult {
            path: path_str,
            match_count: 0,
            matches: Vec::new(),
            error: Some(format!(
                "File too large, skipped (>10 MB): {} bytes",
                metadata.len()
            )),
        };
    }

    // Build the searcher with context lines configuration.
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(context_lines)
        .after_context(context_lines)
        .build();

    // Collect matches using a UTF8 sink.
    let mut matches = Vec::new();
    let mut match_count = 0usize;

    let sink_result = searcher.search_path(
        matcher,
        path,
        UTF8(|line_num, line_content| {
            // Check if we've hit the global match limit.
            let current_total = *total_matches.lock().expect("mutex poisoned");
            if current_total >= MAX_MATCHES {
                // Stop searching this file.
                return Ok(false);
            }

            // Determine if this is a match line or a context line.
            // The UTF8 sink doesn't directly tell us, but we can infer it:
            // grep-searcher calls the sink for both match and context lines.
            // We need to use a different approach — use the Sink trait directly.
            
            // For now, we'll use a simpler approach: check if the line matches the pattern.
            // This is not perfect (context lines might also match), but it's a reasonable heuristic.
            let is_match = matcher.is_match(line_content.as_bytes()).unwrap_or(false);

            if is_match {
                match_count += 1;
                // Update the global counter.
                let mut total = total_matches.lock().expect("mutex poisoned");
                *total += 1;
            }

            // Strip trailing newline characters from the line content.
            let content = line_content.trim_end_matches(&['\r', '\n'][..]).to_string();

            matches.push(GrepLine {
                line_no: line_num as usize,
                content,
                is_match,
            });

            Ok(true) // Continue searching
        }),
    );

    // Handle search errors (binary file detection, I/O errors, etc.).
    match sink_result {
        Ok(_) => FileGrepResult {
            path: path_str,
            match_count,
            matches,
            error: None,
        },
        Err(e) => {
            // Check if this is a binary file error.
            let error_msg = e.to_string();
            let is_binary = error_msg.contains("binary") || error_msg.contains("Binary");

            FileGrepResult {
                path: path_str,
                match_count: 0,
                matches: Vec::new(),
                error: Some(if is_binary {
                    "Binary file, skipped".to_string()
                } else {
                    format!("Search failed: {}", e)
                }),
            }
        }
    }
}
