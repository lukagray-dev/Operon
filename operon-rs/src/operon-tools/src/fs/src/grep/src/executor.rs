/// Executor for the grep tool — handles all file searching and result assembly.
///
/// This module contains the core logic for searching files with regex patterns,
/// applying filename and ignore filters, collecting context lines, enforcing
/// size and match limits, and assembling the final plain-text ToolResult.
///
/// # Output format (plain text)
///
/// GLOB-ONLY MODE (no patterns):
///   "{count} file(s) matched\n\n{path} ({size})\n..."
///
/// SEARCH MODE:
///   "{total} match(es) in {files} file(s){truncated_note}\n\n{per-file blocks}"
///
///   Per-file block:
///     "{absolute_path}\n{line_no}| {content}\n..."
///
///   Files separated by blank lines. Multiple match groups within a file
///   are also separated by blank lines.
///
///   If truncated:
///     "***omitted {remaining} matches***" appended at end.
///
/// Error per file:
///   "{absolute_path}\nERROR: {reason}"
use crate::args::GrepArgs;
use globset::{Glob, GlobSet, GlobSetBuilder};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{SearcherBuilder, Sink, SinkContext, SinkMatch};
use ignore::types::TypesBuilder;
use ignore::WalkBuilder;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use std::path::{Path, PathBuf};

/// Maximum total matches across all files before truncation.
///
/// Once this limit is reached, we stop searching and append a truncation notice.
const MAX_MATCHES: usize = 300;

/// Maximum file size in bytes (10 MB).
///
/// Files larger than this are skipped with an inline error message.
const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// A single line in the search output (match or context).
///
/// Private to this module — used only during collection before text formatting.
struct SearchLine {
    /// 1-indexed line number in the source file.
    line_no: usize,
    /// Line text content, with trailing newline removed.
    content: String,
}

/// All lines collected for one file during search.
///
/// Private to this module.
struct FileSearchResult {
    /// Absolute path of the searched file.
    path: String,
    /// Number of actual match lines (not context) in this file.
    match_count: usize,
    /// All match and context lines in order.
    lines: Vec<SearchLine>,
    /// Optional error message if the file could not be searched.
    error: Option<String>,
}

/// Custom sink for grep-searcher that collects matches and context lines.
///
/// The `Sink` trait provides separate callbacks for matches vs context,
/// which is the only correct way to distinguish them.
struct GrepSink<'a> {
    /// Accumulates all search lines (matches + context) for this file.
    lines: &'a mut Vec<SearchLine>,
    /// Number of actual matches (not context) found so far in this file.
    match_count: &'a mut usize,
    /// Running total across all files — shared reference for limit enforcement.
    total_matches: &'a mut usize,
    /// Stop searching when total_matches reaches this value.
    max_matches: usize,
}

impl<'a> Sink for GrepSink<'a> {
    type Error = std::io::Error;

    /// Called for each line matching the regex pattern.
    fn matched(
        &mut self,
        _searcher: &grep_searcher::Searcher,
        sink_match: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        // Stop if we've hit the global match limit.
        if *self.total_matches >= self.max_matches {
            return Ok(false);
        }

        let line_no = sink_match.line_number().unwrap_or(0) as usize;
        let content = String::from_utf8_lossy(sink_match.bytes())
            .trim_end_matches(&['\r', '\n'][..])
            .to_string();

        self.lines.push(SearchLine { line_no, content });

        *self.match_count += 1;
        *self.total_matches += 1;

        Ok(true)
    }

    /// Called for each context line surrounding a match.
    fn context(
        &mut self,
        _searcher: &grep_searcher::Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, Self::Error> {
        let line_no = ctx.line_number().unwrap_or(0) as usize;
        let content = String::from_utf8_lossy(ctx.bytes())
            .trim_end_matches(&['\r', '\n'][..])
            .to_string();

        self.lines.push(SearchLine { line_no, content });

        Ok(true)
    }
}

/// Executes the grep tool with the given arguments.
///
/// Dispatches to glob-only mode (if patterns is empty) or search mode.
/// Returns ToolContent::Text always — no JSON.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args`: The parsed grep arguments.
///
/// # Returns
/// A `ToolResult` with `is_error: false` always. Errors (invalid regex, file I/O)
/// are embedded inline in the text output.
pub async fn execute(call_id: ToolCallId, args: GrepArgs) -> ToolResult {
    // GLOB-ONLY MODE: no patterns → list matching files without searching content.
    if args.patterns.is_empty() {
        let file_paths =
            collect_file_paths(&args.path, args.glob.as_deref(), &args.ignore);

        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("{} file(s) matched", file_paths.len()));
        lines.push(String::new()); // blank line after header

        for p in &file_paths {
            // Get file size for display.
            let size_str = std::fs::metadata(p)
                .map(|m| human_size(m.len()))
                .unwrap_or_else(|_| "? B".to_string());
            lines.push(format!("{} ({})", p.display(), size_str));
        }

        return ToolResult {
            call_id,
            name: "grep".to_string(),
            content: ToolContent::Text(lines.join("\n")),
            is_error: false,
            read_paths: None,
        };
    }

    // SEARCH MODE: build a single regex matcher from all patterns joined with |.
    // This gives OR semantics: a line matches if ANY pattern matches.
    let combined_pattern = args.patterns.join("|");

    let matcher = match RegexMatcherBuilder::new().build(&combined_pattern) {
        Ok(m) => m,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "grep".to_string(),
                content: ToolContent::Text(format!("ERROR: invalid regex pattern: {}", e)),
                is_error: false,
                read_paths: None,
            };
        }
    };

    // Collect file paths to search.
    let file_paths = collect_file_paths(&args.path, args.glob.as_deref(), &args.ignore);

    // Run the actual search in a blocking task (grep-searcher is sync, not async).
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
                    content: ToolContent::Text(
                        "ERROR: internal error: search task panicked".to_string(),
                    ),
                    is_error: false,
                    read_paths: None,
                };
            }
        };

    // Build summary statistics.
    let total_matches: usize = results.iter().map(|r| r.match_count).sum();
    let files_with_matches = results.iter().filter(|r| r.match_count > 0).count();

    // Count remaining matches not shown (for truncation notice).
    // Simple approximation: MAX_MATCHES is the cap, so at least (total_matches - MAX_MATCHES) are omitted.
    let omitted = if truncated {
        total_matches.saturating_sub(MAX_MATCHES)
    } else {
        0
    };

    // Format the header line.
    let header = if truncated {
        format!(
            "{} match(es) in {} file(s) [truncated]",
            total_matches, files_with_matches
        )
    } else {
        format!(
            "{} match(es) in {} file(s)",
            total_matches, files_with_matches
        )
    };

    // Format each file's results into a text block.
    let mut blocks: Vec<String> = vec![header];

    for result in results {
        let block = format_file_result(result);
        if !block.is_empty() {
            blocks.push(block);
        }
    }

    // Append truncation notice at the very end if needed.
    if truncated {
        blocks.push(format!("***omitted {} matches***", omitted));
    }

    ToolResult {
        call_id,
        name: "grep".to_string(),
        content: ToolContent::Text(blocks.join("\n\n")),
        is_error: false,
        read_paths: None,
    }
}

/// Format one file's search result into a text block.
///
/// Error files:
///   "{path}\nERROR: {reason}"
///
/// Match files (one block per contiguous group of lines):
///   "{path}\n{line_no}| {content}\n..."
///
/// Multiple match groups within a file are separated by a blank line.
/// Files with zero matches and no error produce an empty string (omitted).
fn format_file_result(result: FileSearchResult) -> String {
    // Error case — always show even if no matches.
    if let Some(err) = result.error {
        return format!("{}\nERROR: {}", result.path, err);
    }

    // Skip files with no matches.
    if result.lines.is_empty() {
        return String::new();
    }

    let mut output = result.path.clone();
    output.push('\n');

    // Group consecutive lines together; emit a blank line between non-consecutive groups.
    // A "gap" is when the current line_no != previous line_no + 1.
    let mut prev_line_no: Option<usize> = None;

    for search_line in &result.lines {
        if let Some(prev) = prev_line_no {
            if search_line.line_no > prev + 1 {
                // Gap between lines — separate match groups with a blank line.
                output.push('\n');
            }
        }

        // Format as "{line_no}| {content}\n"
        output.push_str(&format!("{}| {}\n", search_line.line_no, search_line.content));

        prev_line_no = Some(search_line.line_no);
    }

    output
}

/// Collect all file paths under `root_path` matching the glob and ignore filters.
///
/// Uses WalkBuilder (from the `ignore` crate) with standard filters (gitignore, hidden).
/// Returns a sorted, deduplicated list of matching file paths.
///
/// # Arguments
/// - `root_path`: Root directory or file to walk.
/// - `glob`: Optional glob pattern to filter filenames (e.g. "*.py").
/// - `ignore`: Entry names to skip during the walk (matched by globset).
fn collect_file_paths(root_path: &str, glob: Option<&str>, ignore_patterns: &[String]) -> Vec<PathBuf> {
    let root = Path::new(root_path);

    // If root is a direct file, return it immediately (filters don't apply).
    if root.is_file() {
        return vec![root.to_path_buf()];
    }

    // Build the glob type filter for the walker (filters by file extension / name pattern).
    let types_filter = glob.and_then(|g| {
        let mut builder = TypesBuilder::new();
        if builder.add("custom", g).is_ok() {
            builder.select("custom");
            builder.build().ok()
        } else {
            None
        }
    });

    // Build the ignore GlobSet to skip entries by name.
    let ignore_globset = build_globset(ignore_patterns);

    // Set up the directory walker with standard filters (gitignore, hidden files).
    let mut walker_builder = WalkBuilder::new(root);
    walker_builder.standard_filters(true);

    if let Some(ref types) = types_filter {
        walker_builder.types(types.clone());
    }

    let mut paths: Vec<PathBuf> = Vec::new();

    for entry in walker_builder.build().flatten() {
        // Only process files, not directories.
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        // Apply ignore patterns against the entry file name.
        if let Some(name) = entry.path().file_name() {
            if ignore_globset.is_match(name) {
                continue;
            }
        }

        paths.push(entry.into_path());
    }

    // Sort for deterministic ordering across platforms.
    paths.sort();
    paths
}

/// Search all files for the regex pattern and return per-file results.
///
/// Runs synchronously in a blocking task. Enforces MAX_MATCHES across all files.
/// Returns (results, truncated) where truncated=true means MAX_MATCHES was hit.
fn search_files(
    file_paths: Vec<PathBuf>,
    matcher: grep_regex::RegexMatcher,
    context_lines: usize,
) -> (Vec<FileSearchResult>, bool) {
    let mut total_matches: usize = 0;
    let mut truncated = false;
    let mut results = Vec::new();

    for path in file_paths {
        // Stop searching if we've hit the global match limit.
        if total_matches >= MAX_MATCHES {
            truncated = true;
            break;
        }

        let result = search_single_file(&path, &matcher, context_lines, &mut total_matches);

        // Only include files with matches or errors in output.
        if result.match_count > 0 || result.error.is_some() {
            results.push(result);
        }
    }

    if total_matches >= MAX_MATCHES {
        truncated = true;
    }

    (results, truncated)
}

/// Search a single file for the regex pattern and return a FileSearchResult.
///
/// Handles size checking, binary detection via grep-searcher, and match collection.
fn search_single_file(
    path: &Path,
    matcher: &grep_regex::RegexMatcher,
    context_lines: usize,
    total_matches: &mut usize,
) -> FileSearchResult {
    let path_str = path.display().to_string();

    // Check file size before searching to avoid loading huge files.
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            return FileSearchResult {
                path: path_str,
                match_count: 0,
                lines: Vec::new(),
                error: Some(format!("failed to access file: {}", e)),
            };
        }
    };

    if metadata.len() > MAX_FILE_SIZE_BYTES {
        return FileSearchResult {
            path: path_str,
            match_count: 0,
            lines: Vec::new(),
            error: Some(format!(
                "file too large, skipped (>10 MB): {} bytes",
                metadata.len()
            )),
        };
    }

    // Build the searcher with context line configuration.
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(context_lines)
        .after_context(context_lines)
        .build();

    let mut lines: Vec<SearchLine> = Vec::new();
    let mut match_count = 0usize;
    let mut local_total = *total_matches;

    let sink_result = searcher.search_path(
        matcher,
        path,
        GrepSink {
            lines: &mut lines,
            match_count: &mut match_count,
            total_matches: &mut local_total,
            max_matches: MAX_MATCHES,
        },
    );

    // Write back the updated total.
    *total_matches = local_total;

    match sink_result {
        Ok(_) => FileSearchResult {
            path: path_str,
            match_count,
            lines,
            error: None,
        },
        Err(e) => {
            // Check if this is a binary file detection error from grep-searcher.
            let error_msg = e.to_string();
            let is_binary =
                error_msg.to_lowercase().contains("binary");

            FileSearchResult {
                path: path_str,
                match_count: 0,
                lines: Vec::new(),
                error: Some(if is_binary {
                    "binary file, skipped".to_string()
                } else {
                    format!("search failed: {}", e)
                }),
            }
        }
    }
}

/// Build a GlobSet from a list of ignore pattern strings.
///
/// Used to match entry names during directory walk. Returns an empty GlobSet
/// (matches nothing) if the patterns list is empty or any pattern fails to compile.
fn build_globset(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    // If build fails, return an empty GlobSet (no entries ignored).
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Format a file size in bytes as a human-readable string.
///
/// Examples: "512 B", "4.2 KB", "1.8 MB"
fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
