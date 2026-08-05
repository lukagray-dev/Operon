//! Output types for the edit tool.
//!
//! This module defines the structured result format returned by the edit tool
//! on successful completion. Failures use ToolContent::Text directly — no struct needed.

use serde::{Deserialize, Serialize};

/// Top-level output returned to the model on successful edit.
///
/// Only returned when ALL hunks applied and the file was written successfully.
/// All failure cases return `ToolResult { is_error: true, content: ToolContent::Text(...) }`.
#[derive(Debug, Serialize, Deserialize)]
pub struct EditOutput {
    /// The file that was edited (echoed back for correlation).
    pub path: String,

    /// Total number of hunks applied.
    pub hunks_applied: usize,

    /// Human-readable summary: "Applied N edit(s) to path/to/file.ext"
    pub message: String,
}
