//! # operon-tools-load
//!
//! Implements the `load_tools` tool for the Operon agent.
//!
//! `load_tools` returns tool names, short descriptions, and JSON schemas for a named
//! built-in tool group on demand. Call this before using any tool group to discover
//! what tools are available and how to call them.
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_load::{definition, execute_list_groups, execute_with_defs};
//! use operon_context_normalize_tools::ToolCallId;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls load_tools with no group, list all groups
//! let result = execute_list_groups(
//!     ToolCallId("call_123".to_string()),
//!     vec!["fs".to_string(), "shell".to_string()],
//! );
//!
//! // 3. When the model calls load_tools with a group, return tools for that group
//! // (defs would be extracted from the dispatcher)
//! let defs = vec![];
//! let result = execute_with_defs(
//!     ToolCallId("call_456".to_string()),
//!     "fs",
//!     defs,
//! );
//! # }
//! ```

mod args;
mod error;
mod output;

#[cfg(test)]
mod tests;

pub use args::LoadToolsArgs;
pub use error::LoadToolsError;
pub use output::{GroupListOutput, GroupLoadOutput, LoadedTool};

use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolDefinition, ToolResult};
use operon_tools_core::TieredToolDefinition;
use serde_json::json;

/// Returns the tiered tool definition for the `load_tools` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the two call modes.
/// - `detailed`: sent after a malformed call. Full explanation with input shapes,
///   error cases, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    let parameters = json!({
        "type": "object",
        "properties": {
            "group": {
                "type": "string",
                "description": "Tool group to load. Omit to list all available groups."
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "load_tools".to_string(),
            description: "Loads tool definitions for a named group on demand. Pass `group` to get \
                          tool names, descriptions, and schemas for that group. Omit `group` to list \
                          all available groups. Always call this before using any tool — tools are not \
                          available until loaded.".to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "load_tools".to_string(),
            description: "Loads tool definitions for a named group on demand.\n\
                          \n\
                          ## Two call modes\n\
                          \n\
                          1. **List all groups** (no `group` argument):\n\
                             Call `load_tools` with no arguments to see all available tool groups.\n\
                             Returns: `{ available_groups: [\"fs\", \"shell\", ...], message: \"...\" }`\n\
                          \n\
                          2. **Load a specific group** (with `group` argument):\n\
                             Call `load_tools { group: \"fs\" }` to load all tools in the \"fs\" group.\n\
                             Returns: `{ group: \"fs\", tool_count: 7, tools: [ { name, description, parameters }, ... ] }`\n\
                          \n\
                          ## Why tools are loaded on demand\n\
                          \n\
                          Tools are not available until explicitly loaded. This keeps context efficient —\n\
                          loading all tools upfront would bloat every request with hundreds of definitions.\n\
                          Instead, you load only the groups you need, when you need them.\n\
                          \n\
                          ## What each tool definition contains\n\
                          \n\
                          - `name`: The tool's identifier (e.g., \"read\", \"bash\")\n\
                          - `description`: What the tool does and key constraints\n\
                          - `parameters`: JSON Schema describing the tool's arguments\n\
                          \n\
                          Use these to understand how to call each tool correctly.\n\
                          \n\
                          ## Error handling\n\
                          \n\
                          If you pass an unknown group name, load_tools returns an error:\n\
                          `unknown group: 'xyz'. Call load_tools with no arguments to list available groups.`\n\
                          \n\
                          ## Workflow example\n\
                          \n\
                          1. Call `load_tools {}` → see available groups\n\
                          2. Call `load_tools { group: \"fs\" }` → see fs tools\n\
                          3. Use fs tools (read, write, etc.) with confidence\n\
                          \n\
                          ## For extensions (OHub)\n\
                          \n\
                          For installed extensions, use `mcp_load` instead of `load_tools`.\n\
                          `load_tools` is for built-in groups only.".to_string(),
            parameters,
        },
    }
}

/// Called by the dispatcher when a group name was provided.
///
/// `defs` is the pre-extracted list of ToolDefinitions for that group.
/// Returns a ToolResult with the group's tools or an error if the group is unknown.
pub fn execute_with_defs(
    call_id: ToolCallId,
    group: &str,
    defs: Vec<&ToolDefinition>,
) -> ToolResult {
    // If no tools found for this group, return an error.
    if defs.is_empty() {
        return ToolResult {
            call_id,
            name: "load_tools".to_string(),
            content: ToolContent::Text(format!(
                "unknown group: '{}'. Call load_tools with no arguments to list available groups.",
                group
            )),
            is_error: true,
        };
    }

    // Convert tool definitions to LoadedTool entries.
    let tools: Vec<LoadedTool> = defs
        .into_iter()
        .map(|d| LoadedTool {
            name: d.name.clone(),
            description: d.description.clone(),
            parameters: d.parameters.clone(),
        })
        .collect();

    let output = GroupLoadOutput {
        group: group.to_string(),
        tool_count: tools.len(),
        tools,
    };

    ToolResult {
        call_id,
        name: "load_tools".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|e| {
                json!({ "error": format!("serialization bug: {}", e) })
            }),
        ),
        is_error: false,
    }
}

/// Called by the dispatcher when no group was provided — lists all groups.
///
/// Returns a ToolResult with the list of available groups and a helpful message.
pub fn execute_list_groups(
    call_id: ToolCallId,
    groups: Vec<String>,
) -> ToolResult {
    let output = GroupListOutput {
        available_groups: groups.clone(),
        message: format!(
            "Call load_tools with a group name to load its tools. \
             Available: {}",
            groups.join(", ")
        ),
    };

    ToolResult {
        call_id,
        name: "load_tools".to_string(),
        content: ToolContent::Json(
            serde_json::to_value(&output).unwrap_or_else(|e| {
                json!({ "error": format!("serialization bug: {}", e) })
            }),
        ),
        is_error: false,
    }
}
