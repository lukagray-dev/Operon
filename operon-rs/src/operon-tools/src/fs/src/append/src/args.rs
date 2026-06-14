//! Argument types for the append tool.
//!
//! This module defines the manual parsing logic for append tool arguments.
//! Arguments arrive as a serde_json::Value where every value is a String.
//! The dispatcher injects the body content as args_json["__body__"].
//! No serde Deserialize is used — all parsing is done field-by-field.
//!
//! Note: empty content validation is intentionally NOT done here. It is done
//! in executor.rs so the model receives an inline ERROR in the ToolResult output
//! rather than a hard ArgsParse failure.

/// Arguments for the append tool.
///
/// - `path`:    absolute path to the existing file to append to (from the `path` attr).
/// - `content`: the text to append (from the `__body__` field injected by the dispatcher).
///
/// Empty content is allowed here; the executor rejects it with an inline error.
pub struct AppendArgs {
    /// Absolute path to the file to append to. The file must already exist.
    pub path: String,

    /// Text content to append. Comes from the `__body__` field injected by the dispatcher.
    /// May be empty — the executor will reject empty content with an inline ERROR.
    pub content: String,
}

impl AppendArgs {
    /// Parses AppendArgs from the raw args_json Value injected by the dispatcher.
    ///
    /// Returns `Ok(AppendArgs)` on success, or `Err(String)` with a human-readable
    /// reason if the required `path` attr is missing, non-string, or empty.
    ///
    /// # Parsing rules
    /// - `path`:    required; must be a non-empty string under args_json["path"].
    /// - `content`: optional; comes from args_json["__body__"]; defaults to "" if absent.
    ///              Empty content is NOT rejected here — that validation lives in the executor.
    pub fn parse(args_json: &serde_json::Value) -> Result<AppendArgs, String> {
        // Extract the "path" attribute — mandatory, must be a non-empty string.
        let path = args_json
            .get("path")
            .or_else(|| args_json.get("paths"))
            .ok_or_else(|| "missing or non-string attr: path".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'path' must be a string".to_string())?
            .trim()
            .to_string();

        if path.is_empty() {
            return Err("path is empty".to_string());
        }

        // Extract body content injected by the dispatcher under "__body__".
        // Empty body is allowed here — the executor will check and return an inline error.
        let content = args_json["__body__"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(AppendArgs { path, content })
    }
}
