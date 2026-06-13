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
    /// Parse grep tool arguments from the attrs+body JSON produced by the LLM parser.
    ///
    /// Extracts `path` from args_json["path"] and all body options from
    /// args_json["__body__"]. Missing or empty body = glob-only mode with no filters.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - The `path` key is missing or not a string.
    /// - `context` value cannot be parsed as usize.
    pub fn parse(args_json: &serde_json::Value) -> Result<GrepArgs, String> {
        // Step 1: Extract the required "path" attribute.
        let path = args_json
            .get("path")
            .ok_or_else(|| "missing required attribute 'path'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'path' must be a string".to_string())?
            .to_string();

        // Step 2: Extract the optional body string. Empty/missing body = defaults.
        let body = args_json
            .get("__body__")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Step 3: Parse the body into fields.
        let (patterns, glob, ignore, context_lines) = parse_body(body)?;

        Ok(GrepArgs {
            path,
            patterns,
            glob,
            ignore,
            context_lines,
        })
    }
}

/// Parse the body string into grep option fields.
///
/// Body lines have the format: `key=token1 token2 ...`
/// The first `=` separates the key from the values. Tokens are already unquoted
/// by the parser (quotes were stripped before the body was assembled).
///
/// Recognized keys:
///   "pattern"  → push each token to patterns vec
///   "glob"     → first token → glob = Some(token)
///   "ignore"   → push each token to ignore vec
///   "context"  → parse first token as usize → context_lines
///
/// Unknown keys are silently ignored.
///
/// # Errors
/// Returns Err if "context" value is not a valid usize.
fn parse_body(
    body: &str,
) -> Result<(Vec<String>, Option<String>, Vec<String>, usize), String> {
    let mut patterns: Vec<String> = Vec::new();
    let mut glob: Option<String> = None;
    let mut ignore: Vec<String> = Vec::new();
    let mut context_lines: usize = 0;

    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Split on the FIRST '=' only. Left side is key, right side is raw values.
        let eq_pos = match line.find('=') {
            Some(pos) => pos,
            None => continue, // Line with no '=' is ignored.
        };

        let key = line[..eq_pos].trim();
        let values_str = line[eq_pos + 1..].trim();

        let tokens = parse_tokens(values_str);

        match key {
            "pattern" => {
                // Each token is a separate pattern (OR-combined during search).
                for token in tokens {
                    patterns.push(token);
                }
            }
            "glob" => {
                // Only the first token is used.
                if let Some(first) = tokens.first() {
                    glob = Some(first.clone());
                }
            }
            "ignore" => {
                // Each token is a separate ignore pattern.
                for token in tokens {
                    ignore.push(token);
                }
            }
            "context" => {
                // Parse the first token as a usize.
                if let Some(first) = tokens.first() {
                    context_lines = first.parse::<usize>().map_err(|_| {
                        format!("invalid context value '{}': must be a non-negative integer", first)
                    })?;
                }
            }
            // All other keys are silently ignored for forward-compatibility.
            _ => {}
        }
    }

    Ok((patterns, glob, ignore, context_lines))
}

/// Helper to parse space-separated tokens from a line, respecting double quotes and unescaping.
fn parse_tokens(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '"' {
            chars.next(); // consume opening quote
            let mut val = String::new();
            while let Some(next_c) = chars.next() {
                if next_c == '\\' {
                    if let Some(&esc_c) = chars.peek() {
                        if esc_c == '"' || esc_c == '\\' {
                            val.push(esc_c);
                            chars.next();
                            continue;
                        }
                    }
                }
                if next_c == '"' {
                    break;
                }
                val.push(next_c);
            }
            tokens.push(val);
        } else {
            let mut val = String::new();
            while let Some(&next_c) = chars.peek() {
                if next_c.is_whitespace() {
                    break;
                }
                val.push(next_c);
                chars.next();
            }
            tokens.push(val);
        }
    }
    tokens
}
