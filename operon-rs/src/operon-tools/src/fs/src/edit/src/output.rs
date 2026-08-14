//! Output types for the edit tool.
//!
//! Hey friend! This module defines the structured result format returned by the edit tool
//! on execution. It models both complete success and partial-success outcomes, providing
//! precise diagnostics for any hunks that failed to match cleanly.

use serde::{Deserialize, Serialize};

/// Detailed failure diagnostic for a single hunk in an edit call.
///
/// Provides the hunk index, original search text, and the specific failure reason
/// (e.g. not found, ambiguous match count) so the model can regenerate ONLY the failed hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HunkFailure {
    /// 0-based index of the failed hunk in the `edits` array.
    pub hunk_index: usize,

    /// The `old_string` that failed to match cleanly.
    pub old_string: String,

    /// Explanation of why the hunk failed to apply.
    pub reason: String,
}

/// Structured output returned by the edit tool.
///
/// Contains aggregate counts and detailed per-failed-hunk diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditOutput {
    /// The target file that was edited.
    pub path: String,

    /// Total number of hunks requested in `args.edits`.
    pub total_hunks: usize,

    /// Total number of hunks that matched cleanly and were applied.
    pub hunks_applied: usize,

    /// Total number of hunks that failed to match.
    pub hunks_failed: usize,

    /// Detailed diagnostics for each failed hunk (omitted if all hunks succeeded).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub failures: Vec<HunkFailure>,

    /// Human-readable summary of the edit outcome.
    pub message: String,
}
