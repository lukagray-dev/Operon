//! Argument types for the write tool.
//!
//! This module defines the manual parsing logic for write tool arguments.
//! Arguments arrive as a serde_json::Value where every value is a String.
//! The dispatcher injects the body content as args_json["__body__"].
//! No serde Deserialize is used — all parsing is done field-by-field.

/// Arguments for the write tool.
///
/// - `path`: absolute path to the file to create or overwrite (from the `path` attr).
/// - `content`: the full file content to write (from the `__body__` field injected by the dispatcher).
///
/// Content is allowed to be empty — writing an empty file is a valid operation.
pub struct WriteArgs {
    /// Absolute path to the file to create or overwrite.
    pub path: String,

    /// Full content to write to the file.
    /// Comes from the `__body__` field injected by the dispatcher.
    /// Empty string is valid — this writes an empty file.
    pub content: String,
}

impl WriteArgs {
    /// Parses WriteArgs from the raw args_json Value injected by the dispatcher.
    ///
    /// Returns `Ok(WriteArgs)` on success, or `Err(String)` with a human-readable
    /// reason if the required `path` attr is missing, non-string, or empty.
    ///
    /// # Parsing rules
    /// - `path`:    required; must be a non-empty string under args_json["path"].
    /// - `content`: optional; comes from args_json["__body__"]; defaults to "" if absent.
    pub fn parse(args_json: &serde_json::Value) -> Result<WriteArgs, String> {
        // Extract the "path" attribute — mandatory, must be a non-empty string.
        let path = args_json["path"]
            .as_str()
            .ok_or_else(|| "missing or non-string attr: path".to_string())?
            .trim()
            .to_string();

        if path.is_empty() {
            return Err("path is empty".to_string());
        }

        // Extract body content injected by the dispatcher under "__body__".
        // Empty body is allowed — writing an empty file is valid.
        let content = args_json["__body__"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(WriteArgs { path, content })
    }
}
