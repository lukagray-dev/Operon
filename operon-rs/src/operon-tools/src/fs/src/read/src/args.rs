/// Argument types for the read tool.
///
/// Hey friend! This module defines the defensive deserialization schema for the read tool's input.
/// The tool accepts string paths with optional `:start-end` line ranges
/// (e.g., `"src/main.rs:10-40"`, `"src/main.rs:5-EOF"`, `"src/main.rs:15"`, `"src/main.rs"`).
/// It seamlessly handles parameter aliases, stringified arrays, and batch files.
use serde::Deserialize;

/// A single read target — a path with an optional line range.
///
/// Deserializes from path strings containing optional range suffixes (`"src/main.rs:10-40"`,
/// `"src/main.rs:5-EOF"`, `"src/main.rs:15"`, `"src/main.rs"`).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ReadTarget {
    /// Absolute path to the file.
    pub path: String,

    /// Optional start line (1-indexed, inclusive). If omitted, starts from line 1.
    pub start_line: Option<usize>,

    /// Optional end line (1-indexed, inclusive). If omitted, reads to EOF.
    pub end_line: Option<usize>,
}

impl<'de> Deserialize<'de> for ReadTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let path = String::deserialize(deserializer)?;
        Ok(parse_string_target(&path))
    }
}

/// Parses a path string, extracting any trailing `:start-end`, `:start-EOF`, or `:line` range.
///
/// Windows drive letters (`D:\...`) are preserved if the suffix after the last colon
/// is not a valid line range specification.
pub fn parse_string_target(s: &str) -> ReadTarget {
    if let Some(idx) = s.rfind(':') {
        let path_part = &s[..idx];
        let suffix = &s[idx + 1..];

        if let Some((start_str, end_str)) = suffix.split_once('-') {
            if let Ok(start) = start_str.parse::<usize>() {
                if start > 0 {
                    if end_str.eq_ignore_ascii_case("eof") {
                        return ReadTarget {
                            path: path_part.to_string(),
                            start_line: Some(start),
                            end_line: None,
                        };
                    } else if let Ok(end) = end_str.parse::<usize>() {
                        if end >= start {
                            return ReadTarget {
                                path: path_part.to_string(),
                                start_line: Some(start),
                                end_line: Some(end),
                            };
                        }
                    }
                }
            }
        } else if let Ok(line) = suffix.parse::<usize>() {
            if line > 0 {
                return ReadTarget {
                    path: path_part.to_string(),
                    start_line: Some(line),
                    end_line: Some(line),
                };
            }
        }
    }

    ReadTarget {
        path: s.to_string(),
        start_line: None,
        end_line: None,
    }
}

fn deserialize_flexible_targets_opt<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<ReadTarget>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt_list = operon_tools_core::de::deserialize_flexible_string_list_opt(deserializer)?;
    Ok(opt_list.map(|list| list.into_iter().map(|s| parse_string_target(&s)).collect()))
}

/// Top-level args the model sends when calling the `read` tool.
///
/// Accepts:
/// - `{ "path": "D:\\path\\file.rs:10-40" }` — single file with inline range
/// - `{ "path": ["D:\\path\\a.rs:10-40", "D:\\path\\b.rs:5-EOF", "D:\\path\\c.rs"] }` — batch files in single call
/// - `{ "paths": [...] }`, `{ "filePath": "..." }`, `{ "files": [...] }` — defensive aliases
#[derive(Debug, Deserialize)]
pub struct ReadArgs {
    /// File path or array of file paths to read (with optional inline ranges like `"file.rs:10-40"`).
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_targets_opt",
        alias = "paths",
        alias = "file_path",
        alias = "filePath",
        alias = "filepath",
        alias = "files",
        alias = "file_paths",
        alias = "filePaths",
        alias = "target_file",
        alias = "file",
        alias = "filename",
        alias = "targets"
    )]
    pub path: Option<Vec<ReadTarget>>,
}

impl ReadArgs {
    /// Returns total number of targets specified in `path`.
    pub fn target_count(&self) -> usize {
        self.path.as_ref().map_or(0, |p| p.len())
    }

    /// Normalizes inputs into a list of `ReadTarget` items.
    pub fn into_targets(self) -> Vec<ReadTarget> {
        self.path.unwrap_or_default()
    }
}
