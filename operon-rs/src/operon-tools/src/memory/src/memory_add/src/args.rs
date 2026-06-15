//! Argument parsing for the memory_add tool.
//!
//! Parses XML attributes parsed by the AI agent's tool-call processor.
//! E.g., <memory_add content="Saved API token context">.

use serde_json::Value;

/// Holds the parsed arguments for the memory_add tool.
#[derive(Debug)]
pub struct MemoryAddArgs {
    /// The actual memory text content to save.
    pub content: String,
}

impl MemoryAddArgs {
    /// Parses the tool arguments from the JSON object produced by the dispatcher.
    ///
    /// # Errors
    /// Returns a helpful string explanation if:
    /// - The `content` attribute is missing.
    /// - The `content` attribute is not a string.
    /// - The `content` value is blank.
    pub fn parse(args_json: &Value) -> Result<Self, String> {
        let content = args_json
            .get("content")
            .ok_or_else(|| "missing required attribute 'content'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'content' must be a string".to_string())?
            .trim()
            .to_string();

        if content.is_empty() {
            return Err("attribute 'content' cannot be empty".to_string());
        }

        Ok(MemoryAddArgs { content })
    }
}
