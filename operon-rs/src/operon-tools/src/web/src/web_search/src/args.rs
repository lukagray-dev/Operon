//! Argument types for the web_search tool.
//!
//! This module defines manual parsing logic for the web_search tool's plain-text
//! attr-based input format. All attribute values arrive as strings from the custom
//! LLM tool-call parser — no serde Deserialize is used.
//!
//! Call format:
//!   <web_search query="search terms" max="10">
//!
//! `max` is optional; defaults to 5 if absent, capped at 10, minimum 1.

/// Arguments for the web_search tool.
///
/// Constructed via `WebSearchArgs::parse` from the raw serde_json::Value attr map.
/// All attribute values arrive as strings from the dispatcher's plain-text parser.
#[derive(Debug)]
pub struct WebSearchArgs {
    /// The search query. Same syntax as typing into DuckDuckGo.
    /// Supports advanced operators: site:, filetype:, quotes, -exclude, etc.
    pub query: String,

    /// Maximum number of results to return. Optional.
    /// Default: 5. Capped at 10, minimum 1.
    /// Arrives as a string (e.g. "10") since all attrs are strings.
    pub max: Option<usize>,
}

impl WebSearchArgs {
    /// Parses WebSearchArgs from the raw args_json Value produced by the dispatcher.
    ///
    /// All attribute values are strings — `max` is provided as e.g. "10", not 10.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - `query` is missing, not a string, or empty after trimming.
    /// - `max` is present but not parseable as a positive integer string.
    pub fn parse(args_json: &serde_json::Value) -> Result<WebSearchArgs, String> {
        // Extract the required "query" attribute — must be a non-empty string.
        let query = args_json["query"]
            .as_str()
            .ok_or_else(|| "missing or non-string attr: query".to_string())?
            .trim()
            .to_string();

        // Guard: query must not be empty — an empty query is a model mistake.
        if query.is_empty() {
            return Err("query is empty".to_string());
        }

        // Extract the optional "max" attribute.
        // It arrives as a string (e.g. "10"), or may be absent/null.
        let max = match args_json.get("max") {
            // Absent or explicitly null → use the executor's default.
            None | Some(serde_json::Value::Null) => None,
            Some(v) => Some(
                v.as_str()
                    .ok_or_else(|| "max must be a string".to_string())?
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| format!("max is not a valid integer: {:?}", v))?,
            ),
        };

        Ok(WebSearchArgs { query, max })
    }
}
