//! Output types for the append tool.
//!
//! This module defines the structured result format returned by the append tool
//! on successful completion. Failures use ToolContent::Text directly — no struct needed.

use serde::{Deserialize, Serialize};

/// Top-level output returned to the model on successful append.
///
/// Only returned when content was successfully appended.
/// All failure cases return `ToolResult { is_error: true, content: ToolContent::Text(...) }`.
#[derive(Debug, Serialize, Deserialize)]
pub struct AppendOutput {
    /// The file that was appended to (echoed back for correlation).
    pub path: String,

    /// Number of bytes appended.
    pub bytes_appended: usize,

    /// Total file size in bytes after the append.
    pub total_bytes: u64,

    /// Human-readable summary: "Appended N bytes to path/to/file.ext (total: M bytes)"
    pub message: String,
}

impl AppendOutput {
    /// Formats the append output as raw plain text with a status header.
    pub fn to_plain_text(&self) -> String {
        format!(
            "=== {} (appended {} bytes, total {} bytes) ===",
            self.path, self.bytes_appended, self.total_bytes
        )
    }
}

