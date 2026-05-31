//! # operon-tools-web
//!
//! Web tools for the Operon agent: web_search and web_fetch.
//!
//! - `web_search`: Query DuckDuckGo and get structured results (title, URL, snippet).
//! - `web_fetch`: Fetch a URL and return clean markdown content.

pub use operon_tools_web_search as web_search;
pub use operon_tools_web_fetch  as web_fetch;
