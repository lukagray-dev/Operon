//! Argument types for the load_tools tool.
//!
//! Hey friend! Defines the defensive deserialization schema for the load_tools tool's input.
//! Supports single group strings, stringified arrays, array of group names, and parameter aliases.

use operon_tools_core::de::deserialize_flexible_single_string_opt;

/// Arguments for the load_tools tool.
#[derive(Debug, serde::Deserialize)]
pub struct LoadToolsArgs {
    /// Name of the tool group to load. If omitted, returns a list of all
    /// available groups with their descriptions.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_single_string_opt",
        alias = "tool_group",
        alias = "toolGroup",
        alias = "name",
        alias = "groups",
        alias = "group_name",
        alias = "groupName"
    )]
    pub group: Option<String>,
}
