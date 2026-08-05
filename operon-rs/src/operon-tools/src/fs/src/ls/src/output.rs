//! Output types for the ls tool.
//!
//! This module defines the structured result format returned by the ls tool.
//! The output contains a list of directory entries with metadata and type information.

use serde::{Deserialize, Serialize};

/// Type of a directory entry.
///
/// Indicates whether an entry is a regular file, directory, or symbolic link.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum EntryKind {
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Symbolic link.
    Symlink,
}

/// A single entry in a directory listing.
///
/// Contains the entry name, type, and optional metadata (size, modification time).
#[derive(Debug, Serialize, Deserialize)]
pub struct LsEntry {
    /// Entry name (not full path).
    pub name: String,

    /// Entry type: FILE, DIR, or SYMLINK.
    pub kind: EntryKind,

    /// File size in bytes. None for directories and symlinks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,

    /// Last modified timestamp as Unix seconds. None if unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_unix: Option<i64>,
}

/// Top-level output returned to the model.
#[derive(Debug, Serialize, Deserialize)]
pub struct LsOutput {
    /// The directory that was listed (echoed back for correlation).
    pub path: String,

    /// Total number of entries in the listing (after exclusions).
    pub entry_count: usize,

    /// Whether the result was capped (more than MAX_ENTRIES entries exist).
    pub truncated: bool,

    /// Directory entries, sorted: dirs first (alphabetical), then files (alphabetical).
    pub entries: Vec<LsEntry>,

    /// Human-readable error if the path could not be listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

impl LsOutput {
    /// Formats the ls output as plain text directory listing.
    pub fn to_plain_text(&self) -> String {
        if let Some(err) = &self.error {
            return format!("=== {} ===\nError: {}", self.path, err);
        }

        let mut out = String::new();
        let trunc_suffix = if self.truncated { " (truncated)" } else { "" };
        out.push_str(&format!("=== {} ({} items{}) ===\n", self.path, self.entry_count, trunc_suffix));

        if self.entries.is_empty() {
            out.push_str("(empty directory)");
            return out;
        }

        for entry in &self.entries {
            match entry.kind {
                EntryKind::Dir => {
                    out.push_str(&format!("[DIR]  {}/\n", entry.name));
                }
                EntryKind::File => {
                    let size_str = entry.size_bytes.map(format_size).unwrap_or_else(|| "0 B".to_string());
                    out.push_str(&format!("[FILE] {} ({})\n", entry.name, size_str));
                }
                EntryKind::Symlink => {
                    out.push_str(&format!("[LINK] {}\n", entry.name));
                }
            }
        }

        if out.ends_with('\n') {
            out.pop();
        }

        out
    }
}

