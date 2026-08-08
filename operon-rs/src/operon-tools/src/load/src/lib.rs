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
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};
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
            description: "\
Loads tool definitions for a named group on demand.

## Input shapes

1. List available tool groups (no arguments):
   `{}`

2. Load tools for a specific group:
   `{\"group\": \"fs\"}`

## Response format

1. Group list mode returns available group names:
   `{\"available_groups\": [\"fs\", \"shell\"], \"message\": \"...\"}`

2. Group load mode returns tool definitions:
   `{\"group\": \"fs\", \"tool_count\": 7, \"tools\": [{\"name\": \"read\", \"description\": \"...\", \"parameters\": {...}}]}`"
                .to_string(),
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
    execute_with_progress(call_id, group, defs, None)
}

/// Called by the dispatcher when a group name was provided, with optional progress reporting.
pub fn execute_with_progress(
    call_id: ToolCallId,
    group: &str,
    defs: Vec<&ToolDefinition>,
    progress: Option<ToolProgressEmitter>,
) -> ToolResult {
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "load_tools",
            Some(group.to_string()),
            format!("Loading tool group {}", group),
        ),
    );

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
            serde_json::to_value(&output)
                .unwrap_or_else(|e| json!({ "error": format!("serialization bug: {}", e) })),
        ),
        is_error: false,
    }
}

/// Called by the dispatcher when no group was provided — lists all groups.
///
/// Returns a ToolResult with the list of available groups and a helpful message.
pub fn execute_list_groups(call_id: ToolCallId, groups: Vec<String>) -> ToolResult {
    execute_list_groups_with_progress(call_id, groups, None)
}

/// Called by the dispatcher when no group was provided â€” lists all groups, with optional progress reporting.
pub fn execute_list_groups_with_progress(
    call_id: ToolCallId,
    groups: Vec<String>,
    progress: Option<ToolProgressEmitter>,
) -> ToolResult {
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "load_tools",
            None,
            "Listing available tool groups",
        ),
    );

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
            serde_json::to_value(&output)
                .unwrap_or_else(|e| json!({ "error": format!("serialization bug: {}", e) })),
        ),
        is_error: false,
    }
}
