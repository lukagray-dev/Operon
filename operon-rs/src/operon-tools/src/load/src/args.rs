//! Argument types for the load_tools tool.

/// Arguments for the load_tools tool.
#[derive(Debug, serde::Deserialize)]
pub struct LoadToolsArgs {
    /// Name of the tool group to load. If omitted, returns a list of all
    /// available groups with their descriptions.
    #[serde(default)]
    pub group: Option<String>,
}
