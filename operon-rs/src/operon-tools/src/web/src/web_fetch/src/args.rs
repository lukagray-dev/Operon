//! Argument types for the web_fetch tool.
//!
//! This module defines the deserialization schema for the web_fetch tool's input.
//! The tool accepts a URL and an optional timeout in milliseconds.

use serde::Deserialize;

/// Arguments for the web_fetch tool.
///
/// Specifies a URL to fetch and an optional timeout in milliseconds.
/// The URL must be a valid http:// or https:// URL.
#[derive(Debug, Deserialize)]
pub struct WebFetchArgs {
    /// The URL to fetch. Must be a valid http:// or https:// URL.
    /// Relative URLs are not supported — provide the full URL.
    pub url: String,

    /// Optional timeout in milliseconds. Default: 15000 (15 seconds).
    /// Increase for slow sites. No maximum — the model is responsible for
    /// setting a reasonable value for the task.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}
