//! Output types for the write tool.
//!
//! This module defines the structured result format returned by the write tool
//! on successful completion. Failures use ToolContent::Text directly — no struct needed.

use serde::{Deserialize, Serialize};

/// Top-level output returned to the model on successful write.
///
/// Only returned when the file was written successfully.
/// All failure cases return `ToolResult { is_error: true, content: ToolContent::Text(...) }`.
#[derive(Debug, Serialize, Deserialize)]
pub struct WriteOutput {
    /// The file that was written (echoed back for correlation).
    pub path: String,

    /// True if the file was newly created, false if an existing file was overwritten.
    pub created: bool,

    /// Number of bytes written.
    pub bytes_written: usize,

    /// Human-readable summary.
    /// On create:    "Created path/to/file.ext (N bytes)"
    /// On overwrite: "Overwrote path/to/file.ext (N bytes)"
    pub message: String,
}
