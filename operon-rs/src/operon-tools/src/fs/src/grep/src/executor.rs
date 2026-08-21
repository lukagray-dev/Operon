/// Executor for the grep tool — handles all file searching and result assembly.
///
/// This module contains the core logic for searching files with regex patterns,
/// applying filename filters, respecting gitignore rules, collecting context lines,
/// enforcing size and match limits, and assembling the final ToolResult.
use crate::args::GrepArgs;
use crate::output::{FileGrepResult, GrepLine, GrepOutput};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{SearcherBuilder, Sink, SinkContext, SinkMatch};
use ignore::types::TypesBuilder;
use ignore::WalkBuilder;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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

/// Custom sink implementation for grep-searcher that properly distinguishes
/// between match lines and context lines.
///
/// The `Sink` trait provides separate callbacks for matches vs context,
/// which is the only correct way to distinguish them. The `UTF8` sink
/// combines both into a single closure with no way to tell them apart.
struct GrepSink<'a> {
    /// Accumulated match and context lines for this file.
    matches: &'a mut Vec<GrepLine>,
    /// Number of actual matches (not context lines) found in this file.
    match_count: &'a mut usize,
    /// Running total of matches across all files (for limit enforcement).
    total_matches: &'a mut usize,
    /// Maximum total matches before stopping the search.
    max_matches: usize,
}

impl<'a> Sink for GrepSink<'a> {
    type Error = std::io::Error;

    /// Called for each line that matches the regex pattern.
    fn matched(
        &mut self,
        _searcher: &grep_searcher::Searcher,
        sink_match: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        // Check if we've hit the global match limit.
        if *self.total_matches >= self.max_matches {
            return Ok(false); // Stop searching
        }

        // Extract line number and content from the match.
        let line_no = sink_match.line_number().unwrap_or(0) as usize;
        let content = String::from_utf8_lossy(sink_match.bytes())
            .trim_end_matches(&['\r', '\n'][..])
            .to_string();

        // Add the match line to the results.
        self.matches.push(GrepLine {
            line_no,
            content,
            is_match: true,
        });

        // Increment both file-local and global match counters.
        *self.match_count += 1;
        *self.total_matches += 1;

        Ok(true) // Continue searching
    }

    /// Called for each context line (lines surrounding matches).
    fn context(
        &mut self,
        _searcher: &grep_searcher::Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, Self::Error> {
        // Extract line number and content from the context line.
        let line_no = ctx.line_number().unwrap_or(0) as usize;
        let content = String::from_utf8_lossy(ctx.bytes())
            .trim_end_matches(&['\r', '\n'][..])
            .to_string();

        // Add the context line to the results (is_match: false).
        self.matches.push(GrepLine {
            line_no,
            content,
            is_match: false,
        });

        Ok(true) // Continue searching
    }
}

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
    let paths = args.get_paths();
    if paths.is_empty() {
        return ToolResult {
            call_id,
            name: "grep".to_string(),
            content: ToolContent::Text("Error: No search paths provided.".to_string()),
            is_error: true,
        };
    }

    // Hey friend! Operon requires all filesystem tools to receive absolute paths.
    // This keeps the tool layer purely stateless and deterministic without relying
    // on process-wide current working directory (CWD) state.
    for path_str in &paths {
        if !Path::new(path_str).is_absolute() {
            return ToolResult {
                call_id,
                name: "grep".to_string(),
                content: ToolContent::Text(format!(
                    "Path must be an absolute path. Relative paths are not supported: {}",
                    path_str
                )),
                is_error: true,
            };
        }
    }

    let file_paths = match collect_file_paths(&paths, args.include.as_deref()) {
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
    let context_lines = args.context_lines;
    let (results, truncated) =
        match tokio::task::spawn_blocking(move || search_files(file_paths, matcher, context_lines))
            .await
        {
            Ok(r) => r,
            Err(_) => {
                return ToolResult {
                    call_id,
                    name: "grep".to_string(),
                    content: ToolContent::Text("Internal error: search task panicked".to_string()),
                    is_error: true,
                };
            }
        };

    // Assemble the final output structure with summary statistics.
    let total_matches: usize = results.iter().map(|r| r.match_count).sum();
    let files_with_matches = results.iter().filter(|r| r.match_count > 0).count();

    let output = GrepOutput {
        total_matches,
        files_with_matches,
        truncated,
        files: results,
    };

    // Return the successful ToolResult with plain text content.
    ToolResult {
        call_id,
        name: "grep".to_string(),
        content: ToolContent::Text(output.to_plain_text()),
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
                        if e.file_type().is_some_and(|ft| ft.is_file()) {
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
/// A tuple of (results, truncated) where:
/// - results: List of FileGrepResult, one per file that had matches or errors
/// - truncated: true if the search stopped early due to hitting MAX_MATCHES
fn search_files(
    file_paths: Vec<PathBuf>,
    matcher: grep_regex::RegexMatcher,
    context_lines: usize,
) -> (Vec<FileGrepResult>, bool) {
    let mut total_matches: usize = 0;
    let mut truncated = false;
    let mut results = Vec::new();

    for path in file_paths {
        // Check if we've already hit the global match limit.
        // If so, mark as truncated and stop searching additional files.
        if total_matches >= MAX_MATCHES {
            truncated = true;
            break;
        }

        // Search this file and collect the result.
        let result = search_single_file(&path, &matcher, context_lines, &mut total_matches);

        // Only include files that had matches or errors in the output.
        // Files with zero matches and no error are omitted to reduce output size.
        if result.match_count > 0 || result.error.is_some() {
            results.push(result);
        }
    }

    (results, truncated)
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
/// - `total_matches`: Mutable reference to the global match counter for limit enforcement
///
/// # Returns
/// A `FileGrepResult` with either matches or an error message.
fn search_single_file(
    path: &Path,
    matcher: &grep_regex::RegexMatcher,
    context_lines: usize,
    total_matches: &mut usize,
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

    // Collect matches using our custom GrepSink implementation.
    let mut matches: Vec<GrepLine> = Vec::new();
    let mut match_count = 0usize;

    // Dereference to get the current total before the search.
    // We pass a local copy in and write back after.
    let mut local_total = *total_matches;

    let sink_result = searcher.search_path(
        matcher,
        path,
        GrepSink {
            matches: &mut matches,
            match_count: &mut match_count,
            total_matches: &mut local_total,
            max_matches: MAX_MATCHES,
        },
    );

    // Write back the updated total.
    *total_matches = local_total;

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
