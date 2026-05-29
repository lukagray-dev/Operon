//! Per-session read ledger for read-before-write/edit enforcement.
//!
//! Tracks which file paths have been successfully read at least once in the
//! current agent session. The dispatcher consults this ledger before allowing
//! `write` (on existing files) or `edit` to proceed.
//!
//! ## Compaction reset
//!
//! When context compaction fires, the ledger is cleared via `clear()`. This forces
//! the model to re-read files before editing or overwriting them, because the
//! compaction summary may have dropped file content the model previously saw.
//! Without this reset, the model could overwrite a file based on a stale mental
//! model of its contents.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Tracks paths read in the current session.
///
/// One instance per agent session, owned by the `Dispatcher`.
/// All methods take `&mut self` — not designed for concurrent access.
#[derive(Debug, Default)]
pub struct ReadLedger {
    /// Set of canonicalized paths that have been successfully read this session.
    paths: HashSet<PathBuf>,
}

impl ReadLedger {
    /// Creates an empty ledger (no paths read yet).
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `path` was successfully read.
    ///
    /// Called by the dispatcher after a `read` tool call returns successfully
    /// (i.e., `ToolResult.is_error == false` and the path was in the results).
    /// Canonicalizes the path before storing to handle `./` prefixes and
    /// redundant separators. Falls back to the raw path if canonicalization fails
    /// (e.g. the file was deleted between read and record — rare but possible).
    pub fn record_read(&mut self, path: &Path) {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.paths.insert(canonical);
    }

    /// Returns `true` if `path` has been read at least once this session.
    ///
    /// Used by the dispatcher to gate `write` (existing files) and `edit`.
    /// Canonicalizes the path before lookup — same logic as `record_read`.
    pub fn has_been_read(&self, path: &Path) -> bool {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.paths.contains(&canonical)
    }

    /// Clears the ledger.
    ///
    /// Call this when context compaction fires. Forces the model to re-read
    /// files before editing or overwriting them after summarization.
    pub fn clear(&mut self) {
        self.paths.clear();
    }

    /// Returns the number of paths currently in the ledger.
    ///
    /// Primarily for testing and diagnostics.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Returns `true` if no paths have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_new_ledger_is_empty() {
        let ledger = ReadLedger::new();
        assert!(ledger.is_empty());
        assert_eq!(ledger.len(), 0);
    }

    #[test]
    fn test_record_and_check() {
        // Create a real temp file so canonicalize works
        let temp_file = NamedTempFile::new().expect("failed to create temp file");
        let path = temp_file.path();

        let mut ledger = ReadLedger::new();
        ledger.record_read(path);

        assert!(ledger.has_been_read(path));
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn test_unread_path_returns_false() {
        let ledger = ReadLedger::new();
        let unread_path = Path::new("/nonexistent/path/to/file.txt");

        // For a nonexistent path, canonicalize fails and we use the raw path.
        // The ledger is empty, so has_been_read returns false.
        assert!(!ledger.has_been_read(unread_path));
    }

    #[test]
    fn test_clear_resets_ledger() {
        let temp_file1 = NamedTempFile::new().expect("failed to create temp file 1");
        let temp_file2 = NamedTempFile::new().expect("failed to create temp file 2");
        let path1 = temp_file1.path();
        let path2 = temp_file2.path();

        let mut ledger = ReadLedger::new();
        ledger.record_read(path1);
        ledger.record_read(path2);

        assert_eq!(ledger.len(), 2);

        ledger.clear();

        assert!(ledger.is_empty());
        assert!(!ledger.has_been_read(path1));
        assert!(!ledger.has_been_read(path2));
    }

    #[test]
    fn test_canonicalization_consistency() {
        // Create a real temp file
        let temp_file = NamedTempFile::new().expect("failed to create temp file");
        let path = temp_file.path();

        let mut ledger = ReadLedger::new();
        ledger.record_read(path);

        // Look up via the same path — should be true
        assert!(ledger.has_been_read(path));

        // Both paths canonicalize to the same value, so lookup should still work
        let canonical = std::fs::canonicalize(path).expect("canonicalize failed");
        assert!(ledger.has_been_read(&canonical));
    }
}
