//! Output types and text formatting for the glob tool.
//!
//! Hey friend! This module formats the matching file paths into a clean plain text summary.

use serde::{Deserialize, Serialize};

/// Structured output returned by the glob tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobOutput {
    /// The glob pattern used for matching.
    pub pattern: String,
    /// The base directory that was searched.
    pub base_path: String,
    /// List of matched relative paths, sorted alphabetically.
    pub matches: Vec<String>,
    /// Total count of matching paths found before truncation.
    pub total_matches: usize,
    /// Whether the results were truncated due to `max_results`.
    pub truncated: bool,
}

impl GlobOutput {
    /// Formats the glob output into a clean, human-readable and model-friendly text string.
    pub fn to_formatted_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "=== glob(\"{}\") in {} ({} match(es){}) ===\n",
            self.pattern,
            self.base_path,
            self.total_matches,
            if self.truncated { ", truncated" } else { "" }
        ));

        if self.matches.is_empty() {
            out.push_str("(no matching files or directories found)\n");
        } else {
            for m in &self.matches {
                out.push_str(m);
                out.push('\n');
            }
            if self.truncated {
                out.push_str(&format!(
                    "\n[Results truncated at {} matches. Narrow your glob pattern or increase max_results]\n",
                    self.matches.len()
                ));
            }
        }

        out
    }
}

