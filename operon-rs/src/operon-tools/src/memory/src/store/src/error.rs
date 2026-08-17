//! Error types for operon-tools-memory-store.
//!
//! Hey friend! This module defines all the ways the memory store can fail.
//! We use thiserror's `#[from]` to get automatic `?`-operator conversions
//! from sqlx errors, IO errors, and operon-config errors — so callers
//! never need to write manual conversion code.

use thiserror::Error;

/// All errors that can occur in the memory store.
///
/// These wrap lower-level failures from sqlx, std::io, and operon-config
/// so callers can pattern-match on what actually went wrong.
#[derive(Debug, Error)]
pub enum MemoryStoreError {
    /// A SQLite query or connection error from sqlx.
    ///
    /// This covers schema migration failures, row insert/update/delete errors,
    /// and FTS5 index consistency errors. The sqlx::Error message always includes
    /// the original SQLite error code and human-readable description.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// An OS-level I/O error — e.g. failing to create the memory directory.
    ///
    /// This wraps std::io::Error. Occurs when `fs::create_dir_all` fails
    /// (permission denied, disk full, etc.) before opening the database.
    #[error("failed to create memory directory: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to resolve the default config paths from `operon-config`.
    ///
    /// Occurs when `OperonPaths::resolve()` fails (e.g. HOME env var not set).
    /// Using `#[from] ConfigError` means we convert it with `?` automatically.
    #[error("failed to resolve default memory path: {0}")]
    Config(#[from] operon_config::ConfigError),
}
