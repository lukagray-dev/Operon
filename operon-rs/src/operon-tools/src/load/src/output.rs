//! Output types for the load_tools tool.

/// A single tool entry returned for a group load.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LoadedTool {
    /// The tool's name (e.g., "read", "bash").
    pub name: String,
    /// Short description of what the tool does.
    pub description: String,
    /// JSON Schema for the tool's parameters.
    pub parameters: serde_json::Value,
}

/// Output when loading a specific group.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GroupLoadOutput {
    /// The group name that was loaded.
    pub group: String,
    /// Number of tools in this group.
    pub tool_count: usize,
    /// List of tools in the group with their definitions.
    pub tools: Vec<LoadedTool>,
}

/// Output when listing all groups (no group argument provided).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GroupListOutput {
    /// List of all available tool groups.
    pub available_groups: Vec<String>,
    /// Helpful message explaining how to use load_tools.
    pub message: String,
}
