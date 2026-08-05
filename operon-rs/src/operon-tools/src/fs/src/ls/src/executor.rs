//! Executor for the ls tool — handles all directory I/O and result assembly.
//!
//! This module contains the core logic for listing directories, applying glob
//! exclusion patterns, collecting metadata, sorting entries, and assembling the
//! final ToolResult. All file I/O is async via tokio::fs.

use crate::args::LsArgs;
use crate::output::{EntryKind, LsEntry, LsOutput};
use globset::{Glob, GlobSetBuilder};
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::io::ErrorKind;
use std::time::UNIX_EPOCH;

/// Maximum number of entries to return in a single listing.
///
/// If a directory contains more entries than this limit, the listing is truncated
/// and the `truncated` flag is set to true. This prevents overwhelming the model
/// with massive directory listings.
const MAX_ENTRIES: usize = 1000;

/// Executes the ls tool with the given arguments.
///
/// Lists a single directory at the given path, applies glob exclusion patterns,
/// collects metadata, sorts entries (directories first, then files), and assembles
/// a structured ToolResult with per-entry information.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call (from the model's request).
/// - `args`: The deserialized ls arguments containing the path and optional ignore patterns.
///
/// # Returns
/// A `ToolResult` with `is_error: false` (even if the directory couldn't be listed).
/// Directory listing errors are embedded in the LsOutput.error field, not surfaced
/// as a top-level error.
pub async fn execute(call_id: ToolCallId, args: LsArgs) -> ToolResult {
    // Execute the listing and assemble the output.
    let output = list_directory(&args).await;

    // Return the successful ToolResult with plain text content.
    ToolResult {
        call_id,
        name: "ls".to_string(),
        content: ToolContent::Text(output.to_plain_text()),
        is_error: false,
    }
}


/// Lists a directory and returns an LsOutput.
///
/// This function handles all the logic for a single directory listing:
/// - Building glob exclusion patterns
/// - Reading directory entries
/// - Filtering excluded entries
/// - Collecting metadata
/// - Sorting entries
/// - Capping results
/// - Error handling and result assembly
///
/// # Arguments
/// - `args`: The ls arguments containing the path and optional ignore patterns.
///
/// # Returns
/// An `LsOutput` with either success + entries or failure + error message.
async fn list_directory(args: &LsArgs) -> LsOutput {
    let path_str = args.path.clone();

    // Build the glob exclusion set from the ignore patterns.
    let globset = match build_globset(&args.ignore) {
        Ok(gs) => gs,
        Err(err_msg) => {
            // Invalid glob pattern — return error output.
            return LsOutput {
                path: path_str,
                entry_count: 0,
                truncated: false,
                entries: vec![],
                error: Some(err_msg),
            };
        }
    };

    // Attempt to read the directory.
    let mut read_dir = match tokio::fs::read_dir(&path_str).await {
        Ok(rd) => rd,
        Err(e) => {
            // Distinguish between different error types for better error messages.
            let error_msg = match e.kind() {
                ErrorKind::NotFound => format!("path not found: {}", path_str),
                ErrorKind::PermissionDenied => format!("permission denied: {}", path_str),
                ErrorKind::InvalidInput => {
                    // This can happen if the path is a file, not a directory.
                    // Try to check if it's a file.
                    match tokio::fs::metadata(&path_str).await {
                        Ok(metadata) if metadata.is_file() => {
                            format!("path is a file, not a directory: {}", path_str)
                        }
                        _ => format!("failed to read directory: {}", e),
                    }
                }
                _ => {
                    // For other errors, try to check if it's a file.
                    match tokio::fs::metadata(&path_str).await {
                        Ok(metadata) if metadata.is_file() => {
                            format!("path is a file, not a directory: {}", path_str)
                        }
                        _ => format!("failed to read directory: {}", e),
                    }
                }
            };

            return LsOutput {
                path: path_str,
                entry_count: 0,
                truncated: false,
                entries: vec![],
                error: Some(error_msg),
            };
        }
    };

    // Collect entries from the directory.
    let mut entries = Vec::new();

    loop {
        // Read the next entry from the directory.
        let entry = match read_dir.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) => break, // End of directory.
            Err(e) => {
                // Error reading entries — return what we have so far with an error note.
                return LsOutput {
                    path: path_str,
                    entry_count: entries.len(),
                    truncated: false,
                    entries,
                    error: Some(format!("error reading directory entries: {}", e)),
                };
            }
        };

        // Get the entry name as a string.
        let file_name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => {
                // Skip entries with non-UTF-8 names.
                continue;
            }
        };

        // Check if this entry should be excluded by the glob patterns.
        if globset.is_match(&file_name) {
            continue;
        }

        // Get metadata for the entry.
        let metadata = match entry.metadata().await {
            Ok(m) => Some(m),
            Err(_) => {
                // If we can't get metadata, still include the entry but with None for size/modified.
                None
            }
        };

        // Determine the entry kind and collect metadata.
        let (kind, size_bytes, modified_unix) = if let Some(ref m) = metadata {
            let kind = if m.is_dir() {
                EntryKind::Dir
            } else if m.is_file() {
                EntryKind::File
            } else {
                // Symlink or other special file type.
                EntryKind::Symlink
            };

            let size_bytes = if m.is_file() { Some(m.len()) } else { None };

            let modified_unix = m
                .modified()
                .ok()
                .and_then(|st| st.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);

            (kind, size_bytes, modified_unix)
        } else {
            // No metadata available — assume it's a symlink or special file.
            (EntryKind::Symlink, None, None)
        };

        // Create the entry and add it to the list.
        entries.push(LsEntry {
            name: file_name,
            kind,
            size_bytes,
            modified_unix,
        });

        // Check if we've reached the entry limit.
        if entries.len() >= MAX_ENTRIES {
            break;
        }
    }

    // Sort entries: directories first (alphabetical, case-insensitive), then files/symlinks.
    entries.sort_by(|a, b| {
        // Compare by kind first: Dir < File/Symlink.
        let kind_cmp = match (&a.kind, &b.kind) {
            (EntryKind::Dir, EntryKind::Dir) => std::cmp::Ordering::Equal,
            (EntryKind::Dir, _) => std::cmp::Ordering::Less,
            (_, EntryKind::Dir) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        };

        if kind_cmp != std::cmp::Ordering::Equal {
            return kind_cmp;
        }

        // Within the same kind, sort alphabetically (case-insensitive).
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    // Check if we truncated the results.
    let truncated = entries.len() >= MAX_ENTRIES;
    let entry_count = entries.len();

    // Assemble and return the output.
    LsOutput {
        path: path_str,
        entry_count,
        truncated,
        entries,
        error: None,
    }
}

/// Builds a GlobSet from the ignore patterns.
///
/// Returns a GlobSet that can be used to match entry names against the patterns.
/// If any pattern fails to compile, returns an error message.
///
/// # Arguments
/// - `ignore`: Optional list of glob patterns to exclude.
///
/// # Returns
/// - `Ok(GlobSet)` if all patterns compile successfully.
/// - `Err(String)` if any pattern fails to compile.
fn build_globset(ignore: &Option<Vec<String>>) -> Result<globset::GlobSet, String> {
    let mut builder = GlobSetBuilder::new();

    if let Some(patterns) = ignore {
        for pattern in patterns {
            match Glob::new(pattern) {
                Ok(glob) => {
                    builder.add(glob);
                }
                Err(e) => {
                    return Err(format!("invalid ignore pattern '{}': {}", pattern, e));
                }
            }
        }
    }

    builder
        .build()
        .map_err(|e| format!("failed to build glob set: {}", e))
}
