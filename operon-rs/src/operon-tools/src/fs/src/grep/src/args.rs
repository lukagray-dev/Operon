/// Argument types for the grep tool.
///
/// This module defines the manual parsing logic for the grep tool's body-based
/// input format. The `path` attr arrives as args_json["path"]. All search options
/// arrive in args_json["__body__"] as a multi-line key=values string.
///
/// Body format:
///   pattern="calculate_total" "Auth"
///   glob="*.py"
///   ignore="node_modules" ".git"
///   context="3"
///
/// Each line is "key=values" where values are whitespace-separated tokens
/// (already unquoted by the parser). Unknown keys are silently ignored.

/// Parsed args for the grep tool.
///
/// All fields are extracted from the `path` attribute and the `__body__` field
/// of the incoming args JSON. No serde derive — parsing is done manually.
#[derive(Debug)]
pub struct GrepArgs {
    /// Root path to search (from the `path` XML attr).
    /// Must be an absolute path to a directory or file.
    pub path: String,

    /// One or more regex patterns to search for.
    /// Empty vec = glob-only mode (list matching files without searching content).
    /// Multiple patterns are OR-combined: a line matches if ANY pattern matches.
    pub patterns: Vec<String>,

    /// Optional glob filter applied during directory walk (e.g. "*.py", "*.{ts,tsx}").
    /// Only affects directory walks, not direct file paths.
    pub glob: Option<String>,

    /// Directory/file names to ignore during walk.
    /// Matched against entry names (not full paths) using globset.
    pub ignore: Vec<String>,

    /// Number of context lines before and after each match. Default 0.
    pub context_lines: usize,
}

impl GrepArgs {
    /// Parse grep tool arguments from the attrs JSON produced by the LLM parser.
    ///
    /// Extracts `path` from args_json["path"] or args_json["paths"] and all options from
    /// attributes.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - The `path`/`paths` key is missing or not a string.
    /// - `context`/`context_lines` value cannot be parsed as usize.
    pub fn parse(args_json: &serde_json::Value) -> Result<GrepArgs, String> {
        // Step 1: Extract the required "path" (or "paths") attribute.
        let path = args_json
            .get("path")
            .or_else(|| args_json.get("paths"))
            .ok_or_else(|| "missing required attribute 'path'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'path' must be a string".to_string())?
            .to_string();

        // Step 2: Parse patterns
        let mut patterns = Vec::new();
        if let Some(attr_pat) = args_json
            .get("pattern")
            .or_else(|| args_json.get("patterns"))
            .and_then(|v| v.as_str())
        {
            patterns = parse_tokens(attr_pat);
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

        // Step 5: Parse context
        let mut context_lines = 0;
        if let Some(attr_context) = args_json
            .get("context")
            .or_else(|| args_json.get("context_lines"))
            .and_then(|v| v.as_str())
        {
            context_lines = attr_context.parse::<usize>().map_err(|_| {
                format!(
                    "invalid context value '{}': must be a non-negative integer",
                    attr_context
                )
            })?;
        }

        Ok(GrepArgs {
            path,
            patterns,
            glob,
            ignore,
            context_lines,
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
