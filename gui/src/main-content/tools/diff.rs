//! Code diff parser and statistics generator.
//!
//! Provides thread-safe parsing of unified diff outputs from the model's
//! workspace editing tools, preparing them for colorized inline rendering
//! in the GUI.

use serde_json::Value;

/// Represents a single line in a unified diff viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDiffLine {
    /// The style classification: "added", "removed", or "context".
    pub kind: String,

    /// The clean text payload (excluding unified diff prefixes).
    pub text: String,
}

/// Helper to parse tool arguments from either raw JSON or runner-formatted tag stream.
pub fn parse_tool_args_to_value(args_json: &str) -> Value {
    if let Ok(val) = serde_json::from_str::<Value>(args_json) {
        return val;
    }

    // Check if the argument is runner-formatted: first line is JSON, followed by __body__:
    if let Some(idx) = args_json.find("__body__:\n") {
        let first_part = args_json[..idx].trim();
        let body = &args_json[idx + "__body__:\n".len()..];
        if let Ok(mut val) = serde_json::from_str::<Value>(first_part) {
            if let Some(obj) = val.as_object_mut() {
                obj.insert("__body__".to_string(), Value::String(body.to_string()));
                obj.insert("content".to_string(), Value::String(body.to_string()));
            }
            return val;
        }
    }

    serde_json::from_str(args_json).unwrap_or_default()
}

/// Parses tool arguments to extract diff lines and compute modification stats.
///
/// Returns a tuple of:
/// 1. Vector of parsed code lines ready for rendering.
/// 2. Count of added lines.
/// 3. Count of deleted lines.
pub fn parse_diff(tool_name: &str, args_json: &str) -> (Vec<ParsedDiffLine>, i32, i32) {
    let val = parse_tool_args_to_value(args_json);

    match tool_name {
        "write" | "append" => {
            // For write/append, the text is the full content body
            let body = val
                .get("__body__")
                .or_else(|| val.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if body.is_empty() {
                return (Vec::new(), 0, 0);
            }

            let lines: Vec<&str> = body.split('\n').collect();
            let added_count = lines.len() as i32;
            let diff_lines = lines
                .into_iter()
                .map(|line| ParsedDiffLine {
                    kind: "added".to_string(),
                    text: line.to_string(),
                })
                .collect();

            (diff_lines, added_count, 0)
        }
        "edit" => {
            // For edit, the text is a unified diff hunk structure in __body__
            let body = val.get("__body__").and_then(|v| v.as_str()).unwrap_or("");

            if body.is_empty() {
                return (Vec::new(), 0, 0);
            }

            let mut diff_lines = Vec::new();
            let mut added = 0;
            let mut deleted = 0;

            for line in body.split('\n') {
                // Per user request, skip @@ hunk descriptor lines to keep card visual presentation clean.
                if line.starts_with("@@") {
                    continue;
                }

                if line.starts_with('+') {
                    added += 1;
                    diff_lines.push(ParsedDiffLine {
                        kind: "added".to_string(),
                        text: line[1..].to_string(),
                    });
                } else if line.starts_with('-') {
                    deleted += 1;
                    diff_lines.push(ParsedDiffLine {
                        kind: "removed".to_string(),
                        text: line[1..].to_string(),
                    });
                } else if line.starts_with(' ') {
                    diff_lines.push(ParsedDiffLine {
                        kind: "context".to_string(),
                        text: line[1..].to_string(),
                    });
                } else if !line.trim().is_empty() {
                    // Fallback context line if it didn't start with space prefix
                    diff_lines.push(ParsedDiffLine {
                        kind: "context".to_string(),
                        text: line.to_string(),
                    });
                }
            }

            (diff_lines, added, deleted)
        }
        _ => (Vec::new(), 0, 0),
    }
}

/// Helper to extract diff data and attach diff lines/stats to a tool card item.
pub fn apply_diff_overlay(
    item: &mut crate::main_content::markdown::ParsedMarkdownItem,
    name: &str,
    args_json: &str,
) {
    if matches!(name, "write" | "edit" | "append") {
        let (lines, added, deleted) = parse_diff(name, args_json);
        if !lines.is_empty() {
            item.tool_is_diff = true;
            item.tool_diff_lines = lines;
            item.tool_added_count = added;
            item.tool_deleted_count = deleted;
        }
    }
}

