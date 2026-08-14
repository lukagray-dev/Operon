//! Argument types for the web_fetch tool.
//!
//! Hey friend! Defines the defensive deserialization schema for the web_fetch tool's input.
//! Supports URL synonyms and numeric string timeout parsing.

use operon_tools_core::de::deserialize_flexible_u64_opt;
use serde::Deserialize;

/// Arguments for the web_fetch tool.
///
/// Specifies a URL to fetch and an optional timeout in milliseconds.
/// The URL must be a valid http:// or https:// URL.
#[derive(Debug, Deserialize)]
pub struct WebFetchArgs {
    /// The URL to fetch. Must be a valid http:// or https:// URL.
    /// Relative URLs are not supported — provide the full URL.
    #[serde(
        alias = "uri",
        alias = "link",
        alias = "address",
        alias = "target"
    )]
    pub url: String,

    /// Optional timeout in milliseconds. Default: 15000 (15 seconds).
    /// Increase for slow sites. Capped at 60000 (60 seconds).
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_u64_opt",
        alias = "timeout",
        alias = "timeoutMs"
    )]
    pub timeout_ms: Option<u64>,
}
