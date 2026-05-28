use std::path::PathBuf;

use thiserror::Error;

/// All errors produced by snapshot construction.
#[derive(Debug, Error)]
pub enum SnapshotError {
    /// Propagated OS/file-system errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Propagated libgit2 errors.
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    /// Propagated watcher backend errors.
    #[error("Watcher error: {0}")]
    Watcher(#[from] notify::Error),

    /// Returned when the configured project root does not exist.
    #[error("Root path does not exist: {0}")]
    InvalidRoot(PathBuf),
}
