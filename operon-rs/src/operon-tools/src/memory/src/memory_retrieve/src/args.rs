//! Argument parsing for the memory_retrieve tool.
//!
//! Parses XML attributes parsed by the AI agent's tool-call processor.
//! E.g., <memory_retrieve id="42">.

use serde_json::Value;

/// Holds the parsed arguments for the memory_retrieve tool.
#[derive(Debug)]
pub struct MemoryRetrieveArgs {
    /// The numeric ID of the memory to retrieve.
    pub id: i64,
}

impl MemoryRetrieveArgs {
    /// Parses the tool arguments from the JSON object.
    ///
    /// # Errors
    /// Returns a helpful string explanation if:
    /// - The `id` attribute is missing.
    /// - The `id` attribute is not a string.
    /// - The `id` value is not a valid 64-bit integer.
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

        Ok(MemoryRetrieveArgs { id })
    }
}
