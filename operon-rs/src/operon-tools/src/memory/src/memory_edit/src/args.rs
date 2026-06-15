//! Argument parsing for the memory_edit tool.
//!
//! Parses XML attributes parsed by the AI agent's tool-call processor.
//! E.g., <memory_edit id="42" content="Updated value text">.

use serde_json::Value;

/// Holds the parsed arguments for the memory_edit tool.
#[derive(Debug)]
pub struct MemoryEditArgs {
    /// The numeric ID of the memory to edit.
    pub id: i64,

    /// The new content string to write.
    pub content: String,
}

impl MemoryEditArgs {
    /// Parses the tool arguments from the JSON object.
    ///
    /// # Errors
    /// Returns a helpful string explanation if:
    /// - `id` or `content` attributes are missing.
    /// - `id` or `content` are not strings.
    /// - `id` is not a valid 64-bit integer, or `content` is empty.
    pub fn parse(args_json: &Value) -> Result<Self, String> {
        let id_str = args_json
            .get("id")
            .ok_or_else(|| "missing required attribute 'id'".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'id' must be a string".to_string())?
            .trim();

        let id = id_str
            .parse::<i64>()
            .map_err(|_| format!("attribute 'id' must be a valid number, got '{}'", id_str))?;

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

        Ok(MemoryEditArgs { id, content })
    }
}
