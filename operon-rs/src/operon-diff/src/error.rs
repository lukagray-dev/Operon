// error.rs — Error types for operon-diff
//
// Hey friend! This module defines the custom error types returned by all Git operations
// in the operon-diff crate. Every failure is cleanly mapped into a variant of `DiffError`
// using `thiserror` so caller applications (like Slint desktop UI) receive descriptive,
// human-friendly error messages instead of raw panics or uninformative numeric codes.

use thiserror::Error;

/// Custom Error type representing any failures encountered during Git operations.
#[derive(Debug, Error)]
pub enum DiffError {
    /// Errors originating directly from underlying `libgit2` calls.
    #[error("Git libgit2 error: {0}")]
    Git(#[from] git2::Error),

    /// Standard I/O failures (file system reads/writes, directory manipulation).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Returned when no Git repository can be discovered in the given directory hierarchy.
    #[error("No git repository found in workspace hierarchy: {0}")]
    NoRepository(String),

    /// Returned when resolving the HEAD reference fails or branch points to invalid OID.
    #[error("HEAD commit resolution failed: {0}")]
    HeadResolution(String),

    /// Errors originating from background Tokio task execution (`spawn_blocking` joins).
    #[error("Async task execution error: {0}")]
    TaskJoin(String),

    /// Returned when `user.name` or `user.email` is missing from Git configuration during commit operations.
    #[error("Git signature missing (user.name or user.email not configured): {0}")]
    SignatureMissing(String),

    /// Returned when a requested repository root path cannot be found or opened.
    #[error("Repository not found: {0}")]
    RepoNotFound(String),

    /// Returned when looking up a specific local or remote branch fails.
    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    /// Returned when remote authentication (SSH keys or HTTPS credentials) fails during push/fetch.
    #[error("Remote authentication failed: {0}")]
    RemoteAuth(String),

    /// Returned when a pull or fast-forward merge cannot be performed due to conflicting changes.
    #[error("Merge conflict encountered: {0}")]
    MergeConflict(String),
}

/// Helper implementation to easily wrap `tokio::task::JoinError` into `DiffError::TaskJoin`.
impl From<tokio::task::JoinError> for DiffError {
    fn from(err: tokio::task::JoinError) -> Self {
        DiffError::TaskJoin(err.to_string())
    }
}
