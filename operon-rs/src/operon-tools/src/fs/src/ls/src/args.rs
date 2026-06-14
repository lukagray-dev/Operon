//! Argument types for the ls tool.
//!
//! This module defines the manual parsing logic for the ls tool's body-based
//! input format. The `path` attr arrives as args_json["path"]. All options
//! arrive in args_json["__body__"] as a multi-line key=values string.
//!
//! Body format:
//!   depth="2"
//!   glob="*.py"
//!   ignore="node_modules" ".git"
//!
//! Each line is "key=values" where values are whitespace-separated tokens
//! (already unquoted by the parser). Unknown keys are silently ignored.

/// Parsed args for the ls tool.
///
/// All fields are extracted from the `path` attribute and the `__body__` field
/// of the incoming args JSON. No serde derive — parsing is done manually.
#[derive(Debug)]
pub struct LsArgs {
    /// Absolute path to the directory to list.
    pub path: String,

    /// Tree depth. 1 = single level (default). 0 = unlimited recursion.
    /// depth=2 means immediate children and their immediate children.
    pub depth: usize,

    /// Optional glob filter on file names (not applied to directory names).
    pub glob: Option<String>,

    /// Entry names to ignore during the walk.
    /// Matched against entry names (not full paths). Dirs matching ignore are
    /// also excluded from recursion.
    pub ignore: Vec<String>,
}

impl LsArgs {
    /// Parse ls tool arguments from the attrs JSON produced by the LLM parser.
    ///
    /// Extracts `path` from args_json["path"] and all options from attributes.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - The `path` key is missing or not a string.
    /// - The `depth` value cannot be parsed as usize.
    pub fn parse(args_json: &serde_json::Value) -> Result<LsArgs, String> {
        // Step 1: Extract the required "path" (or "paths") attribute.
        let path = args_json
            .get("path")
            .or_else(|| args_json.get("paths"))
            .ok_or_else(|| "missing required attribute 'path'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'path' must be a string".to_string())?
            .to_string();

        // Step 2: Parse depth
        let mut depth: Option<usize> = None;
        if let Some(attr_depth) = args_json.get("depth").and_then(|v| v.as_str()) {
            depth = Some(attr_depth.parse::<usize>().map_err(|_| {
                format!(
                    "invalid depth value '{}': must be a non-negative integer",
                    attr_depth
                )
            })?);
        }

        // Step 3: Parse glob
        let glob = args_json
            .get("glob")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Step 4: Parse ignore
        let mut ignore = Vec::new();
        if let Some(attr_ignore) = args_json.get("ignore").and_then(|v| v.as_str()) {
            ignore = parse_tokens(attr_ignore);
        }

        Ok(LsArgs {
            path,
            // depth=0 means unlimited. Default is 1 (single level).
            depth: depth.unwrap_or(1),
            glob,
            ignore,
        })
    }
}

/// Helper to parse newline-separated tokens, respecting any internal spaces.
fn parse_tokens(s: &str) -> Vec<String> {
    if s.contains('\n') {
        s.split('\n')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    } else {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            Vec::new()
        } else {
            vec![trimmed.to_string()]
        }
    }
}
