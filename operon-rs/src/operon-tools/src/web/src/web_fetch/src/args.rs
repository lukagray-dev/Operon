//! Argument types for the web_fetch tool.
//!
//! This module defines manual parsing logic for the web_fetch tool's plain-text
//! attr-based input format. All attribute values arrive as strings from the custom
//! LLM tool-call parser — no serde Deserialize is used.
//!
//! Call format:
//!   <web_fetch url="https://example.com">
//!
//! Single required attr: `url`. timeout is managed internally by the HTTP client.

/// Arguments for the web_fetch tool.
///
/// Constructed via `WebFetchArgs::parse` from the raw serde_json::Value attr map.
/// All attribute values arrive as strings from the dispatcher's plain-text parser.
#[derive(Debug)]
pub struct WebFetchArgs {
    /// The URL to fetch. Must start with http:// or https://.
    /// Relative URLs are not supported — provide the full URL.
    pub url: String,
}

impl WebFetchArgs {
    /// Parses WebFetchArgs from the raw args_json Value produced by the dispatcher.
    ///
    /// All attribute values are strings — `url` must start with http:// or https://.
    ///
    /// # Errors
    /// Returns `Err(String)` if:
    /// - `url` is missing or not a string.
    /// - `url` is empty after trimming.
    /// - `url` does not start with http:// or https://.
    pub fn parse(args_json: &serde_json::Value) -> Result<WebFetchArgs, String> {
        // Extract the required "url" attribute — must be a non-empty string.
        let url = args_json
            .get("url")
            .or_else(|| args_json.get("urls"))
            .or_else(|| args_json.get("path"))
            .or_else(|| args_json.get("paths"))
            .ok_or_else(|| "missing or non-string attr: url".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'url' must be a string".to_string())?
            .trim()
            .to_string();

        // Guard: url must not be empty.
        if url.is_empty() {
            return Err("url is empty".to_string());
        }

        // Guard: url must be an http or https URL — ftp:// and others are not supported.
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("url must start with http:// or https://".to_string());
        }

        Ok(WebFetchArgs { url })
    }
}
