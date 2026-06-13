//! # operon-tools-load
//!
//! Implements the `load_tools` tool for the Operon agent.
//!
//! `load_tools` returns plain-text descriptions for every tool in a named built-in
//! group on demand. Call this before using any tool group to discover what tools
//! are available and how to call them. Each tool's description includes its call
//! format and body protocol in plain English — no raw JSON schemas are returned.
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_load::{definition, execute_list_groups, execute_with_defs};
//! use operon_context_normalize::tools::{ToolCallId, ToolDefinition};
//!
//! # fn example() {
//! // 1. Get the tool definition to register with the model.
//! let def = definition();
//!
//! // 2. When the model calls load_tools with no group, list all groups.
//! let result = execute_list_groups(
//!     ToolCallId("call_123".to_string()),
//!     vec!["fs".to_string(), "shell".to_string()],
//! );
//!
//! // 3. When the model calls load_tools with a group, return tools for that group.
//! // (defs would be extracted from the dispatcher via definitions_for_group())
//! let defs: Vec<&ToolDefinition> = vec![];
//! let result = execute_with_defs(
//!     ToolCallId("call_456".to_string()),
//!     "fs",
//!     defs,
//! );
//! # }
//! ```

mod args;

#[cfg(test)]
mod tests;

// Re-export args type for callers that parse args externally.
pub use args::LoadToolsArgs;

use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `load_tools` tool.
///
/// - `short`: Sent to the model under normal conditions. Concise — states what
///   the tool does and the two call modes.
/// - `detailed`: Sent after a malformed call. Full explanation with call modes,
///   output format, worked examples, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "load_tools".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}

/// Called by the dispatcher when a group name was provided.
///
/// `defs` is the pre-extracted list of short `ToolDefinition`s for that group,
/// as returned by `Dispatcher::definitions_for_group()`. Returns a `ToolResult`
/// with plain-text descriptions of each tool, or an error if the group is unknown.
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
    // Emit a progress event so the UI can show "Loading tool group fs…" while waiting.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "load_tools",
            Some(group.to_string()),
            format!("Loading tool group {}", group),
        ),
    );

    // If no tools found for this group, the group name is unknown — return an error.
    if defs.is_empty() {
        return ToolResult {
            call_id,
            name: "load_tools".to_string(),
            content: ToolContent::Text(format!(
                "unknown group: '{}'. Call load_tools with no arguments to list available groups.",
                group
            )),
            is_error: true,
            read_paths: None,
        };
    }

    // Build plain-text output: one "## tool_name\n{description}" section per tool,
    // joined with a blank line between sections for readability.
    let tool_count = defs.len();
    let sections: Vec<String> = defs
        .into_iter()
        .map(|d| {
            format!("## {}\n{}", d.name, d.description)
        })
        .collect();

    let body = sections.join("\n\n");
    let text = format!(
        "Loaded {} tool(s) from group '{}':\n\n{}",
        tool_count, group, body
    );

    ToolResult {
        call_id,
        name: "load_tools".to_string(),
        content: ToolContent::Text(text),
        is_error: false,
        read_paths: None,
    }
}

/// Called by the dispatcher when no group was provided — lists all available groups.
///
/// Returns a plain-text `ToolResult` listing all registered group names and an
/// example of how to load one.
pub fn execute_list_groups(call_id: ToolCallId, groups: Vec<String>) -> ToolResult {
    execute_list_groups_with_progress(call_id, groups, None)
}

/// Called by the dispatcher when no group was provided — lists all groups, with optional
/// progress reporting.
pub fn execute_list_groups_with_progress(
    call_id: ToolCallId,
    groups: Vec<String>,
    progress: Option<ToolProgressEmitter>,
) -> ToolResult {
    // Emit a progress event so the UI shows "Listing available tool groups" while waiting.
    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "load_tools",
            None,
            "Listing available tool groups",
        ),
    );

    // Comma-separated list of group names (e.g. "ask, fs, shell, web").
    let groups_list = groups.join(", ");

    // Pick the first group name (if any) for the usage hint example.
    let example_group = groups.first().cloned().unwrap_or_else(|| "fs".to_string());

    let text = format!(
        "Available groups: {}\n\nCall load_tools with a group name to load its tools, \
         e.g. <load_tools group=\"{}\">",
        groups_list, example_group
    );

    ToolResult {
        call_id,
        name: "load_tools".to_string(),
        content: ToolContent::Text(text),
        is_error: false,
        read_paths: None,
    }
}
