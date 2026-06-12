//! Argument types for the delete tool.
//!
//! This module defines the manual parsing logic for the delete tool's body-based
//! input format. The `path` attr arrives as args_json["path"]. The optional
//! `permanent` flag arrives in args_json["__body__"].
//!
//! Body format (optional):
//!   permanent="true"
//!
//! If the body is absent or empty, permanent defaults to false (trash mode).

/// Parsed args for the delete tool.
///
/// Extracted from the `path` attribute and the `__body__` field of the incoming
/// args JSON. No serde derive — parsing is done manually.
#[derive(Debug)]
pub struct DeleteArgs {
    /// Absolute path to the file or directory to delete.
    /// The path must exist — if it does not, the tool returns an inline error.
    pub path: String,

    /// If true, permanently delete with no recovery possible.
    /// If false (default), move the target to the system trash (recoverable).
    /// Prefer false unless permanent deletion is explicitly required.
    pub permanent: bool,
}

impl DeleteArgs {
    /// Parse delete tool arguments from the attrs+body JSON produced by the LLM parser.
    ///
    /// Extracts `path` from args_json["path"] and the optional `permanent` flag
    /// from args_json["__body__"]. Missing or empty body = permanent defaults to false.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - The `path` key is missing or not a string.
    /// - The `permanent` body value is not "true" or "false".
    pub fn parse(args_json: &serde_json::Value) -> Result<DeleteArgs, String> {
        // Step 1: Extract the required "path" attribute.
        let path = args_json
            .get("path")
            .ok_or_else(|| "missing required attribute 'path'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'path' must be a string".to_string())?
            .to_string();

        // Step 2: Extract the optional body string. Missing/empty = use defaults.
        let body = args_json
            .get("__body__")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Step 3: Parse the body for the "permanent" key.
        let mut permanent: Option<bool> = None;

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

            // Only care about the first token.
            let first_token = values_str.split_whitespace().next().unwrap_or("");

            if key == "permanent" {
                match first_token {
                    "true" => permanent = Some(true),
                    "false" => permanent = Some(false),
                    other => {
                        return Err(format!(
                            "invalid permanent value '{}': must be \"true\" or \"false\"",
                            other
                        ));
                    }
                }
            }
            // Unknown keys are silently ignored for forward-compatibility.
        }

        Ok(DeleteArgs {
            path,
            permanent: permanent.unwrap_or(false),
        })
    }
}
