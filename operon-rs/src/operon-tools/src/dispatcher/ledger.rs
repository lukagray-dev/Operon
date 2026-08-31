//! Read-before-write/edit ledger tracking for the tool dispatcher.
//!
//! Hey friend! This module inspects the output of the `read` tool and registers verified
//! read paths in the `ReadLedger` so subsequent `edit` and `write` calls are allowed.

use operon_context_normalize_tools::{ToolContent, ToolResult};
use operon_tools_core::ReadLedger;

/// Extracts successfully-read file paths from a `read` tool result and
/// records them in the ledger.
///
/// The `read` tool returns either plain text with header banners or a JSON object.
/// Only paths that were successfully read are recorded.
pub(crate) fn record_read_paths(ledger: &mut ReadLedger, result: &ToolResult) {
    match &result.content {
        ToolContent::Text(text) => {
            // Parse plain-text section headers produced by ReadOutput::to_plain_text().
            // Header format: "=== <path> (...) ===" or "=== <path> ==="
            // If the section failed, the next line starts with "Error:".
            let lines: Vec<&str> = text.lines().collect();
            let mut idx = 0;
            while idx < lines.len() {
                let line = lines[idx];
                if line.starts_with("=== ") && line.ends_with(" ===") {
                    let header_inner = &line[4..line.len() - 4];
                    // Extract path (everything before optional space or '(')
                    let path_str = if let Some(space_idx) = header_inner.find(" (") {
                        &header_inner[..space_idx]
                    } else {
                        header_inner
                    };

                    // Check if next line is an error line
                    let has_error = if idx + 1 < lines.len() {
                        lines[idx + 1].starts_with("Error:")
                    } else {
                        false
                    };

                    if !has_error {
                        ledger.record_read(std::path::Path::new(path_str));
                    }
                }
                idx += 1;
            }
        }
        ToolContent::Json(v) => {
            let files = match v.get("files").and_then(|f| f.as_array()) {
                Some(arr) => arr,
                None => return,
            };

            for file in files {
                let is_success = file
                    .get("success")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);

                if is_success {
                    if let Some(path_str) = file.get("path").and_then(|p| p.as_str()) {
                        ledger.record_read(std::path::Path::new(path_str));
                    }
                }
            }
        }
    }
}

