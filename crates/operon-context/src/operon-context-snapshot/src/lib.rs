//! # operon-context-snapshot
//!
//! A standalone snapshot crate that builds the agent system message for each turn.
//!
//! This crate is intentionally a leaf crate: it has no dependency on any
//! `operon-*` crate and can be used independently outside the Operon workspace.
//!
//! The snapshot always contains four plain-text blocks in fixed order:
//! 1. Bootstrap block (agent identity, role, session id, RFC3339 timestamp)
//! 2. Full `AGENTS.md` content (or none)
//! 3. Gitignore-aware project tree (top-level plus configurable depth)
//! 4. Git status summary (omitted when the root is not a git repo)
//!
//! Refresh model:
//! - Bootstrap and git status are recomputed on every `build()`.
//! - `AGENTS.md` and directory tree are cached and refreshed only when a
//!   filesystem watcher marks them dirty.
//!
//! `SnapshotBuilder::build()` is the single entry point for producing a
//! `SessionSnapshot`.

mod blocks;
mod builder;
mod error;
mod types;

pub use builder::{SnapshotBuilder, SnapshotConfig};
pub use error::SnapshotError;
pub use types::{BootstrapBlock, DirectoryTree, GitStatus, Role, SessionSnapshot};

pub type Result<T> = std::result::Result<T, SnapshotError>;
