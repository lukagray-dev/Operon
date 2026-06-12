/// Argument types for the read tool.
///
/// This module defines the manual parsing logic for the read tool's plain-text
/// attr-based input format. Args arrive as a serde_json::Value object where
/// every value is a string (from the custom LLM tool-call parser).
///
/// The `paths` value is a SINGLE STRING — a whitespace-separated list of file
/// path entries. Each entry is either:
///   - "C:\path\to\file.txt"            → full file read
///   - "C:\path\to\file.txt:40-90"      → lines 40 to 90 inclusive (1-indexed)
///   - "C:\path\to\file.txt:50-"        → line 50 to EOF
///   - "C:\path\to\file.txt:-30"        → line 1 to line 30
///
/// Each entry is a single whitespace-delimited token. The parser strips quotes
/// before joining multiple quoted values with a space into args_json["paths"].

/// A single parsed read target from the `paths` attribute.
///
/// Represents one file to read, with an optional 1-indexed inclusive line range.
/// Both `start_line` and `end_line` are None when the full file should be read.
#[derive(Debug)]
pub struct ReadTarget {
    /// Absolute or relative path to the file to read.
    pub path: String,

    /// Optional start line (1-indexed, inclusive). None means start from line 1.
    pub start_line: Option<usize>,

    /// Optional end line (1-indexed, inclusive). None means read to EOF.
    pub end_line: Option<usize>,
}

/// Parsed args for the read tool.
///
/// Contains the list of read targets, each with a path and optional line range.
/// Constructed via `ReadArgs::parse` from the raw serde_json::Value attr map.
#[derive(Debug)]
pub struct ReadArgs {
    /// List of files to read — each is a path with an optional line range.
    pub targets: Vec<ReadTarget>,
}

impl ReadArgs {
    /// Parse the read tool's arguments from the attr map produced by the custom
    /// LLM tool-call parser.
    ///
    /// The parser passes all attributes as string values in a JSON object, so
    /// we extract `args_json["paths"]` as a &str and split on whitespace.
    /// Multiple quoted values in the original call are joined with a space by
    /// the dispatcher before arriving here.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - The `paths` key is missing from `args_json`
    /// - The `paths` value is not a string
    /// - Any individual entry fails to parse (empty path, invalid range, etc.)
    pub fn parse(args_json: &serde_json::Value) -> Result<ReadArgs, String> {
        // Step 1: Extract the "paths" attribute as a string.
        let paths_str = args_json
            .get("paths")
            .ok_or_else(|| "missing required attribute 'paths'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'paths' must be a string".to_string())?;

        // Step 2: Split on whitespace — each token is one path entry (already
        // unquoted by the parser). Empty tokens from extra whitespace are skipped.
        let raw_entries: Vec<&str> = paths_str
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .collect();

        if raw_entries.is_empty() {
            return Err("'paths' is empty — provide at least one file path".to_string());
        }

        // Step 3: Parse each entry into a ReadTarget.
        let mut targets = Vec::with_capacity(raw_entries.len());
        for entry in raw_entries {
            match parse_entry(entry) {
                Ok(target) => targets.push(target),
                Err(reason) => {
                    return Err(format!("invalid path entry '{}': {}", entry, reason));
                }
            }
        }

        Ok(ReadArgs { targets })
    }
}

/// Parse a single path entry string into a ReadTarget.
///
/// A path entry is either:
///   - A plain file path (e.g. "C:\foo\bar.txt")
///   - A path with a line range (e.g. "C:\foo\bar.txt:40-90")
///
/// The range colon is identified as the LAST colon in the entry where what
/// follows matches `^\d*-\d*$` (both sides optional digits, hyphen required).
/// Drive-letter colons (e.g. "C:") are at index 1 followed by a backslash —
/// they never match the range pattern.
fn parse_entry(entry: &str) -> Result<ReadTarget, String> {
    // Scan from right to left for a colon whose suffix matches \d*-\d*.
    // We want the LAST such colon to handle paths that contain colons internally
    // (only drive-letter colons in practice, but we are thorough).
    let range_colon_idx = find_range_colon(entry);

    let (path, start_line, end_line) = if let Some(colon_idx) = range_colon_idx {
        // Split into path and range string at the range colon.
        let path_part = &entry[..colon_idx];
        let range_part = &entry[colon_idx + 1..];

        // Parse the range "START-END" — each side is optional.
        let (start, end) = parse_range(range_part)?;

        // Validate: if both bounds are present, start must not exceed end.
        if let (Some(s), Some(e)) = (start, end) {
            if s > e {
                return Err(format!(
                    "invalid range {}–{}: start line must not exceed end line",
                    s, e
                ));
            }
        }

        (path_part.to_string(), start, end)
    } else {
        // No range found — read the whole file.
        (entry.to_string(), None, None)
    };

    // Path must not be empty after stripping the range.
    if path.trim().is_empty() {
        return Err("path is empty".to_string());
    }

    Ok(ReadTarget {
        path,
        start_line,
        end_line,
    })
}

/// Find the index of the range colon in `entry`, if one exists.
///
/// We scan the entry from right to left. For each colon we find, we check
/// whether the suffix (everything after the colon) matches `^\d*-\d*$`.
/// If it does — AND the colon is NOT at index 1 (drive letter position) —
/// we return that colon's index. The first (rightmost) match wins.
///
/// Returns None if no range colon is found.
fn find_range_colon(entry: &str) -> Option<usize> {
    let bytes = entry.as_bytes();

    // Walk backwards through the string byte by byte looking for colons.
    let mut i = bytes.len().saturating_sub(1);
    loop {
        if bytes[i] == b':' {
            // Guard: drive-letter colon is at index 1, followed by a backslash or forward slash.
            // Drive letters like "C:\" should never be treated as range separators.
            let is_drive_letter = i == 1
                && bytes.len() > 2
                && (bytes[2] == b'\\' || bytes[2] == b'/');

            if !is_drive_letter {
                // Check if suffix matches \d*-\d$ (both digit groups optional, hyphen required).
                let suffix = &entry[i + 1..];
                if is_range_suffix(suffix) {
                    return Some(i);
                }
            }
        }

        if i == 0 {
            break;
        }
        i -= 1;
    }
    None
}

/// Returns true if `s` matches `^\d*-\d*$`:
/// zero or more digits, a hyphen, zero or more digits.
/// The hyphen is required.
fn is_range_suffix(s: &str) -> bool {
    // Must contain exactly one hyphen, nothing else besides digits.
    match s.find('-') {
        None => false,
        Some(dash_idx) => {
            let before = &s[..dash_idx];
            let after = &s[dash_idx + 1..];
            // Both sides must be all digits (empty strings are fine — that means None).
            before.chars().all(|c| c.is_ascii_digit())
                && after.chars().all(|c| c.is_ascii_digit())
        }
    }
}

/// Parse a range string of the form "START-END" where each side is optional.
///
/// Returns `(start_line, end_line)` as `(Option<usize>, Option<usize>)`.
/// Empty side = None (start of file or EOF respectively).
///
/// # Errors
/// Returns Err if a non-empty side fails to parse as usize.
fn parse_range(range_str: &str) -> Result<(Option<usize>, Option<usize>), String> {
    // We know the range_str already matched `\d*-\d*` via is_range_suffix,
    // so find the hyphen and split.
    let dash_idx = range_str
        .find('-')
        .ok_or_else(|| "range must contain a '-'".to_string())?;

    let start_str = &range_str[..dash_idx];
    let end_str = &range_str[dash_idx + 1..];

    // Parse start side — empty means None (start of file).
    let start = if start_str.is_empty() {
        None
    } else {
        Some(
            start_str
                .parse::<usize>()
                .map_err(|_| format!("invalid start line '{}'", start_str))?,
        )
    };

    // Parse end side — empty means None (EOF).
    let end = if end_str.is_empty() {
        None
    } else {
        Some(
            end_str
                .parse::<usize>()
                .map_err(|_| format!("invalid end line '{}'", end_str))?,
        )
    };

    Ok((start, end))
}
