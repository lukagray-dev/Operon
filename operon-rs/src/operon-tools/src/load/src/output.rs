//! Output types for the `load_tools` tool.
//!
//! These structs are kept for compatibility with the existing `tests.rs` (which is
//! being rewritten separately). They are no longer used by the production execution
//! path — `lib.rs` now formats plain-text output directly without these types.
//!
//! Once `tests.rs` is rewritten, this module and its re-exports from `lib.rs` can
//! be removed entirely.

/// A single tool entry — kept for test compatibility only.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LoadedTool {
    /// The tool's name (e.g., "read", "bash").
    pub name: String,
    /// Short description of what the tool does.
    pub description: String,
    /// JSON Schema for the tool's parameters.
    pub parameters: serde_json::Value,
}

/// Output shape for loading a specific group — kept for test compatibility only.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GroupLoadOutput {
    /// The group name that was loaded.
    pub group: String,
    /// Number of tools in this group.
    pub tool_count: usize,
    /// List of tools in the group.
    pub tools: Vec<LoadedTool>,
}

/// Output shape for listing all groups — kept for test compatibility only.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GroupListOutput {
    /// All available tool group names.
    pub available_groups: Vec<String>,
    /// Helpful hint message.
    pub message: String,
}
