//! Output types for the web_fetch tool.
//!
//! Output is now plain text built directly in executor.rs — no JSON structs needed.
//!
//! The type below is a compatibility stub kept only so existing tests.rs can
//! compile until tests are rewritten to match the new plain-text output format.
//! It will be removed when tests.rs is updated.

use serde::{Deserialize, Serialize};

/// Output returned to the model (compatibility stub — output format is now plain text).
///
/// Kept only so tests.rs compiles until it is rewritten.
#[derive(Debug, Serialize, Deserialize)]
pub struct WebFetchOutput {
    /// The URL that was fetched.
    pub url: String,
    /// HTTP status code.
    pub status_code: u16,
    /// Page title extracted from <title> tag, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Page content as clean markdown, truncated to MAX_CONTENT_CHARS.
    pub content: String,
    /// True if content was truncated.
    pub truncated: bool,
    /// Content length in characters (after truncation).
    pub content_length: usize,
}
