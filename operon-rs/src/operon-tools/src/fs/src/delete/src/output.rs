//! Output types for the delete tool.
//!
//! This module defines the structured result format returned by the delete tool
//! on successful completion. Failures use ToolContent::Text directly — no struct needed.

use serde::{Deserialize, Serialize};

/// The kind of filesystem entry that was deleted.
///
/// Indicates whether the deleted target was a file or directory.
/// This helps the model understand what was removed.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeletedKind {
    /// A regular file or symlink was deleted.
    File,
    /// A directory (and all its contents) was deleted.
    Dir,
}

/// Top-level output returned to the model on successful delete.
///
/// Only returned when deletion succeeded.
/// All failure cases return `ToolResult { is_error: true, content: ToolContent::Text(...) }`.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteOutput {
    /// The path that was deleted (echoed back for correlation).
    pub path: String,

    /// Whether the target was a file or directory.
    pub kind: DeletedKind,

    /// Whether the deletion was permanent (true) or moved to trash (false).
    pub permanent: bool,

    /// Human-readable summary.
    /// Trash:     "Moved {path} to trash (file|dir)"
    /// Permanent: "Permanently deleted {path} (file|dir)"
    pub message: String,
}
