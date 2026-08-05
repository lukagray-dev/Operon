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
///
/// Contains the directory listing results, including entries, metadata, and
/// any errors that occurred during listing.
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
    /// When populated, entries is empty and entry_count is 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
