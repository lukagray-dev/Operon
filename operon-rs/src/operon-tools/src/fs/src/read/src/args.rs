/// Argument types for the read tool.
///
/// This module defines the deserialization schema for the read tool's input.
/// The tool accepts either plain path strings or objects with optional line ranges.
use serde::Deserialize;

/// A single read target — a path with an optional line range.
///
/// This type supports two input shapes via untagged deserialization:
/// 1. Plain string: `"src/main.rs"` — reads the entire file
/// 2. Object with optional range: `{"path": "src/main.rs", "start_line": 100, "end_line": 200}`
///
/// The untagged deserialization is implemented via a private intermediate enum.
#[derive(Debug)]
pub struct ReadTarget {
    /// Absolute or relative path to the file.
    pub path: String,

    /// Optional start line (1-indexed, inclusive). If omitted, starts from line 1.
    pub start_line: Option<usize>,

    /// Optional end line (1-indexed, inclusive). If omitted, reads to EOF (subject to size limit).
    pub end_line: Option<usize>,
}

/// Internal helper for untagged deserialization of ReadTarget.
///
/// Serde tries to deserialize into Str first (plain string), then falls back
/// to Obj (object with path + optional line range fields).
#[derive(Deserialize)]
#[serde(untagged)]
enum RawTarget {
    /// Plain string path — reads entire file.
    Str(String),
    /// Object with path and optional line range.
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
        // Deserialize into the intermediate RawTarget enum, then convert to ReadTarget.
        let raw = RawTarget::deserialize(deserializer)?;
        Ok(match raw {
            // Plain string — no line range specified.
            RawTarget::Str(path) => ReadTarget {
                path,
                start_line: None,
                end_line: None,
            },
            // Object with optional line range.
            RawTarget::Obj(obj) => ReadTarget {
                path: obj.path,
                start_line: obj.start_line,
                end_line: obj.end_line,
            },
        })
    }
}

/// Top-level args the model sends when calling the `read` tool.
///
/// Accepts two shapes:
/// - `{ "paths": ["a.rs", "b.rs"] }` — shorthand, reads entire files
/// - `{ "paths": [{ "path": "a.rs", "start_line": 100, "end_line": 200 }] }` — with line ranges
///
/// The `paths` array can mix plain strings and objects.
#[derive(Debug, Deserialize)]
pub struct ReadArgs {
    /// List of files to read. Each entry is either a plain path string or an
    /// object with path + optional start_line/end_line.
    pub paths: Vec<ReadTarget>,
}
