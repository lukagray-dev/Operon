//! Argument parsing for the memory_search tool.
//!
//! Parses XML attributes parsed by the AI agent's tool-call processor.
//! E.g., <memory_search query="api key">.

use serde_json::Value;

/// Holds the parsed arguments for the memory_search tool.
#[derive(Debug)]
pub struct MemorySearchArgs {
    /// The search query to locate matching memories.
    pub query: String,
}

impl MemorySearchArgs {
    /// Parses the tool arguments from the JSON object.
    ///
    /// If the `query` field is missing, null, or empty, it defaults to an empty string,
    /// which indicates to the executor that we want to list all memories.
    ///
    /// # Errors
    /// Returns a helpful string error if:
    /// - The `query` attribute is present but is not a string value (e.g. an array, number, or boolean).
    pub fn parse(args_json: &Value) -> Result<Self, String> {
        // Retrieve the query value from the JSON payload.
        let query = match args_json.get("query") {
            // If the query attribute is absent or null, treat it as an empty query string.
            None | Some(Value::Null) => String::new(),
            // If it is present, ensure it is a string. If not, return a parsing error.
            Some(v) => {
                v.as_str()
                    .ok_or_else(|| "attribute 'query' must be a string".to_string())?
                    .trim()
                    .to_string()
            }
        };

        Ok(MemorySearchArgs { query })
    }
}

