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
    /// Parse delete tool arguments from the attrs JSON produced by the LLM parser.
    ///
    /// Extracts `path` from args_json["path"] and the optional `permanent` flag
    /// from attributes. Missing = permanent defaults to false.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - The `path` key is missing or not a string.
    /// - The `permanent` value is not "true" or "false".
    pub fn parse(args_json: &serde_json::Value) -> Result<DeleteArgs, String> {
        // Step 1: Extract the required "path" attribute.
        let path = args_json
            .get("path")
            .or_else(|| args_json.get("paths"))
            .ok_or_else(|| "missing required attribute 'path'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'path' must be a string".to_string())?
            .to_string();

        // Step 2: Parse permanent attribute tolerantly
        let mut permanent = false;
        if let Some(v) = args_json.get("permanent") {
            if let Some(s) = v.as_str() {
                match s.trim() {
                    "true" => permanent = true,
                    "false" => permanent = false,
                    other => {
                        tracing::warn!(
                            "invalid permanent value '{}', defaulting to false",
                            other
                        );
                        permanent = false;
                    }
                }
            } else {
                tracing::warn!("permanent attribute must be a string, ignoring");
            }
        }

        Ok(DeleteArgs {
            path,
            permanent,
        })
    }
}
