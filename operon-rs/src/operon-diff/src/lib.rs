// lib.rs — Git Diff & Index Engine for Operon
//
// Hey friend! This crate implements the core Git integration for Operon.
// It uses `libgit2` (via `git2` and `auth-git2`) to manage multi-repository registries,
// query unstaged and staged file diffs, commit changes, branch tracking, visual commit graph
// history, remote push/fetch/pull operations, and non-blocking Tokio async wrappers for UI integration.

pub mod branch;
pub mod commit;
pub mod diff;
pub mod dto;
pub mod error;
pub mod graph;
pub mod remote;
pub mod repo_manager;
pub mod stage;
pub mod status;
pub mod workspace;

// Re-export all public types, DTOs, errors, and functions for easy consumption
pub use branch::*;
pub use commit::*;
pub use diff::*;
pub use dto::*;
pub use error::*;
pub use graph::*;
pub use remote::*;
pub use repo_manager::*;
pub use stage::*;
pub use status::*;
pub use workspace::*;
