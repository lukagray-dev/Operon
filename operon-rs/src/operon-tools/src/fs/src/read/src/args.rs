/// Argument types for the read tool.
///
/// This module defines the deserialization schema for the read tool's input.
/// The tool accepts root-level single file parameters or a list of path targets
/// (which can be string paths with optional `:start-end` ranges or objects).
use serde::Deserialize;

/// A single read target — a path with an optional line range.
///
/// Supports string paths with optional range suffixes (`"src/main.rs:10-40"`,
/// `"src/main.rs:5-EOF"`, `"src/main.rs:15"`, `"src/main.rs"`) and objects
/// with explicit `path`, `start_line`, and `end_line` fields.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ReadTarget {
    /// Absolute or relative path to the file.
    pub path: String,

    /// Optional start line (1-indexed, inclusive). If omitted, starts from line 1.
    pub start_line: Option<usize>,

    /// Optional end line (1-indexed, inclusive). If omitted, reads to EOF.
    pub end_line: Option<usize>,
}

/// Internal helper for untagged deserialization of ReadTarget.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawTarget {
    /// Plain string path — may contain `:start-end`, `:start-EOF`, or `:line` suffix.
    Str(String),
    /// Object with path and optional line range fields.
    Obj(ReadTargetObj),
}

/// The object variant of a read target, with explicit fields.
#[derive(Deserialize)]
struct ReadTargetObj {
    /// Absolute or relative path to the file.
    path: String,
    /// Optional start line (1-indexed, inclusive).
    start_line: Option<usize>,
    /// Optional end line (1-indexed, inclusive).
    end_line: Option<usize>,
}

impl<'de> Deserialize<'de> for ReadTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawTarget::deserialize(deserializer)?;
        Ok(match raw {
            RawTarget::Str(path) => parse_string_target(&path),
            RawTarget::Obj(obj) => {
                let mut target = parse_string_target(&obj.path);
                if obj.start_line.is_some() {
                    target.start_line = obj.start_line;
                }
                if obj.end_line.is_some() {
                    target.end_line = obj.end_line;
                }
                target
            }
        })
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

/// Top-level args the model sends when calling the `read` tool.
///
/// Accepts:
/// - `{ "path": "a.rs", "start_line": 10, "end_line": 40 }` — single file
/// - `{ "paths": ["a.rs:10-40", "b.rs:5-EOF", "c.rs"] }` — batch with string ranges
/// - `{ "paths": [{ "path": "a.rs", "start_line": 10, "end_line": 40 }] }` — batch objects
#[derive(Debug, Deserialize)]
pub struct ReadArgs {
    /// Optional single file path passed at top level.
    pub path: Option<String>,
    /// Optional start line for single file path.
    pub start_line: Option<usize>,
    /// Optional end line for single file path.
    pub end_line: Option<usize>,

    /// List of files to read. Each entry can be a string path (with optional `:start-end` suffix)
    /// or an object with `path` + optional `start_line`/`end_line`.
    pub paths: Option<Vec<ReadTarget>>,
}

impl ReadArgs {
    /// Returns total number of targets specified across `path` and `paths`.
    pub fn target_count(&self) -> usize {
        let root_count = usize::from(self.path.is_some());
        let paths_count = self.paths.as_ref().map_or(0, |p| p.len());
        root_count + paths_count
    }

    /// Normalizes inputs into a list of `ReadTarget` items.
    pub fn into_targets(self) -> Vec<ReadTarget> {

        let mut targets = Vec::new();

        if let Some(path_str) = self.path {
            let mut target = parse_string_target(&path_str);
            if self.start_line.is_some() {
                target.start_line = self.start_line;
            }
            if self.end_line.is_some() {
                target.end_line = self.end_line;
            }
            targets.push(target);
        }

        if let Some(paths) = self.paths {
            targets.extend(paths);
        }

        targets
    }
}

