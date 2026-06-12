//! Executor for the ls tool — handles all directory I/O and result assembly.
//!
//! This module contains the core logic for listing directories recursively,
//! applying depth limits, glob filtering on file names, ignore patterns on all
//! entries, sorting, and assembling the final plain-text ToolResult.
//!
//! # Output format
//!
//! ```text
//! [DIR]  src
//! [DIR]  src/utils
//! [FILE] src/utils/math.py (4.2 KB)
//! [FILE] src/utils/helpers.py (9.3 KB)
//! [DIR]  src/api
//! [FILE] src/api/orders.py (2.1 KB)
//! ```
//!
//! Paths are relative to the root path argument.
//! Dirs come before files at each level.
//! Files show human-readable sizes. Dirs do not.
//! Capped at 1000 entries total.

use crate::args::LsArgs;
use globset::{Glob, GlobSet, GlobSetBuilder};
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::path::Path;

/// Maximum number of entries to return before truncating.
const MAX_ENTRIES: usize = 1000;

/// A single line in the ls output (before formatting).
///
/// Private to this module.
struct LsLine {
    /// Path relative to the root, using forward slashes for readability.
    relative_path: String,
    /// true = directory, false = file.
    is_dir: bool,
    /// File size in bytes. None for directories.
    size_bytes: Option<u64>,
}

/// Executes the ls tool with the given arguments.
///
/// Lists the directory at `args.path` up to `args.depth` levels deep,
/// applying glob filtering on file names and ignore patterns on all entries.
/// Returns plain-text output with [DIR] and [FILE] prefixes.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args`: The parsed ls arguments.
///
/// # Returns
/// A `ToolResult` with `is_error: false` always. Errors (non-existent path,
/// file as path) are embedded inline in the text output.
pub async fn execute(call_id: ToolCallId, args: LsArgs) -> ToolResult {
    let root = std::path::PathBuf::from(&args.path);

    // Verify the path exists and is a directory.
    let meta = match tokio::fs::metadata(&root).await {
        Ok(m) => m,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "ls".to_string(),
                content: ToolContent::Text(format!("{}\nERROR: {}", args.path, e)),
                is_error: false,
                read_paths: None,
            };
        }
    };

    if !meta.is_dir() {
        return ToolResult {
            call_id,
            name: "ls".to_string(),
            content: ToolContent::Text(format!(
                "{}\nERROR: path is a file, not a directory",
                args.path
            )),
            is_error: false,
            read_paths: None,
        };
    }

    // Build the ignore GlobSet (matched against entry names, not full paths).
    let ignore_globset = build_globset(&args.ignore);

    // Build the optional glob filter for file names.
    let file_globset: Option<GlobSet> = args.glob.as_deref().and_then(|g| {
        let mut builder = GlobSetBuilder::new();
        if let Ok(glob) = Glob::new(g) {
            builder.add(glob);
            builder.build().ok()
        } else {
            None
        }
    });

    // Collect all directory entries recursively up to the depth limit.
    // depth=0 means unlimited (represented as usize::MAX in the recursive call).
    let max_depth = if args.depth == 0 {
        usize::MAX
    } else {
        args.depth
    };

    // We run the directory walk in a blocking task since std::fs is used.
    let root_clone = root.clone();
    let path_str = args.path.clone();

    let lines = tokio::task::spawn_blocking(move || {
        let mut collected: Vec<LsLine> = Vec::new();
        list_dir(
            &root_clone,
            &root_clone,
            1,
            max_depth,
            file_globset.as_ref(),
            &ignore_globset,
            &mut collected,
        );
        collected
    })
    .await
    .unwrap_or_default();

    // Check if we need to truncate.
    let total = lines.len();
    let truncated = total >= MAX_ENTRIES;
    let shown_lines = if truncated {
        &lines[..MAX_ENTRIES]
    } else {
        &lines[..]
    };

    // Format each line as "[DIR]  relative/path" or "[FILE] relative/path (size)".
    let mut output_parts: Vec<String> = Vec::with_capacity(shown_lines.len() + 2);

    // Header showing the root path.
    output_parts.push(path_str);

    for line in shown_lines {
        if line.is_dir {
            output_parts.push(format!("[DIR]  {}", line.relative_path));
        } else {
            let size_str = line
                .size_bytes
                .map(human_size)
                .unwrap_or_else(|| "? B".to_string());
            output_parts.push(format!("[FILE] {} ({})", line.relative_path, size_str));
        }
    }

    // Append truncation notice if needed.
    if truncated {
        let omitted = total - MAX_ENTRIES;
        output_parts.push(format!("***omitted {} entries***", omitted));
    }

    ToolResult {
        call_id,
        name: "ls".to_string(),
        content: ToolContent::Text(output_parts.join("\n")),
        is_error: false,
        read_paths: None,
    }
}

/// Recursively list the directory at `current`, collecting entries into `out`.
///
/// Entries at each level are sorted: directories first (alphabetical,
/// case-insensitive), then files (alphabetical, case-insensitive).
///
/// # Arguments
/// - `root`: The root path (used to compute relative paths for output).
/// - `current`: The directory currently being listed.
/// - `depth`: Current depth level (1-indexed from root's children).
/// - `max_depth`: Maximum depth to recurse. usize::MAX = unlimited.
/// - `glob`: Optional GlobSet to filter file names (dirs are not filtered).
/// - `ignore`: GlobSet to skip entries entirely (both files and dirs).
/// - `out`: Accumulator for output lines.
fn list_dir(
    root: &Path,
    current: &Path,
    depth: usize,
    max_depth: usize,
    glob: Option<&GlobSet>,
    ignore: &GlobSet,
    out: &mut Vec<LsLine>,
) {
    // Stop early if we've already hit the entry cap.
    if out.len() >= MAX_ENTRIES {
        return;
    }

    // Read directory entries synchronously (we're inside spawn_blocking).
    let read_dir = match std::fs::read_dir(current) {
        Ok(rd) => rd,
        Err(_) => return, // Silently skip unreadable directories.
    };

    // Collect all entries from this level.
    let mut dirs: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut files: Vec<(String, std::path::PathBuf, Option<u64>)> = Vec::new();

    for entry_result in read_dir {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue, // Skip unreadable entries.
        };

        let file_name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue, // Skip non-UTF-8 names.
        };

        // Apply ignore patterns against the entry name.
        if ignore.is_match(&file_name) {
            continue;
        }

        let entry_path = entry.path();
        let metadata = entry.metadata().ok();

        let is_dir = metadata.as_ref().map_or(false, |m| m.is_dir());

        if is_dir {
            dirs.push((file_name, entry_path));
        } else {
            // It's a file (or symlink to file).
            let size_bytes = metadata.as_ref().and_then(|m| {
                if m.is_file() {
                    Some(m.len())
                } else {
                    None
                }
            });

            // Apply glob filter to file names only.
            if let Some(g) = glob {
                if !g.is_match(&file_name) {
                    continue; // File doesn't match the glob filter — skip.
                }
            }

            files.push((file_name, entry_path, size_bytes));
        }
    }

    // Sort dirs alphabetically (case-insensitive).
    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    // Sort files alphabetically (case-insensitive).
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    // Emit dirs first, then files (at this level).
    for (_, dir_path) in &dirs {
        if out.len() >= MAX_ENTRIES {
            return;
        }

        // Compute relative path from root to this directory.
        let rel = relative_path(root, dir_path);

        out.push(LsLine {
            relative_path: rel,
            is_dir: true,
            size_bytes: None,
        });

        // Recurse if we haven't hit the depth limit.
        if depth < max_depth {
            list_dir(root, dir_path, depth + 1, max_depth, glob, ignore, out);
        }
    }

    for (_, file_path, size_bytes) in &files {
        if out.len() >= MAX_ENTRIES {
            return;
        }

        let rel = relative_path(root, file_path);

        out.push(LsLine {
            relative_path: rel,
            is_dir: false,
            size_bytes: *size_bytes,
        });
    }
}

/// Compute the relative path from `root` to `target`, using forward slashes.
///
/// Falls back to the target's full display path if stripping the root prefix fails.
fn relative_path(root: &Path, target: &Path) -> String {
    target
        .strip_prefix(root)
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| target.display().to_string())
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
