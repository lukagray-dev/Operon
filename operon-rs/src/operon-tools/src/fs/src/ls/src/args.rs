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
    /// Parse ls tool arguments from the attrs+body JSON produced by the LLM parser.
    ///
    /// Extracts `path` from args_json["path"] and all body options from
    /// args_json["__body__"]. Missing or empty body = use all defaults.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - The `path` key is missing or not a string.
    /// - The `depth` value cannot be parsed as usize.
    pub fn parse(args_json: &serde_json::Value) -> Result<LsArgs, String> {
        // Step 1: Extract the required "path" attribute.
        let path = args_json
            .get("path")
            .ok_or_else(|| "missing required attribute 'path'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'path' must be a string".to_string())?
            .to_string();

        // Step 2: Extract the optional body string. Empty/missing body = all defaults.
        let body = args_json
            .get("__body__")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Step 3: Parse the body for recognized keys.
        let mut depth: Option<usize> = None;
        let mut glob: Option<String> = None;
        let mut ignore: Vec<String> = Vec::new();

        for raw_line in body.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            // Split on the FIRST '=' only.
            let eq_pos = match line.find('=') {
                Some(pos) => pos,
                None => continue,
            };

            let key = line[..eq_pos].trim();
            let values_str = line[eq_pos + 1..].trim();

            // Tokens are whitespace-separated, already unquoted by the parser.
            let tokens: Vec<&str> = values_str.split_whitespace().collect();

            match key {
                "depth" => {
                    if let Some(first) = tokens.first() {
                        depth = Some(first.parse::<usize>().map_err(|_| {
                            format!(
                                "invalid depth value '{}': must be a non-negative integer",
                                first
                            )
                        })?);
                    }
                }
                "glob" => {
                    if let Some(first) = tokens.first() {
                        glob = Some(first.to_string());
                    }
                }
                "ignore" => {
                    for token in tokens {
                        ignore.push(token.to_string());
                    }
                }
                // Unknown keys are silently ignored for forward-compatibility.
                _ => {}
            }
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
