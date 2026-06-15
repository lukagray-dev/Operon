//! Tool groups block — lists available built-in tool groups in the system message.

/// Renders the tool groups block, or returns `None` if `groups` is empty.
///
/// Injected as the 5th block in the system message every turn.
/// Tells the model which groups are available and how to load them.
pub fn render_tool_groups(groups: &[String]) -> Option<String> {
    if groups.is_empty() {
        return None;
    }

    let group_list = groups.join(", ");

    Some(format!(
        "## Available Tool Groups\n\
         Tools are not loaded by default. You must call `load_tools` with a group name to load and register that group's tools.\n\
         \n\
         ### Tool Call Formatting Protocol\n\
         IMPORTANT: All tool calls must be written in the custom XML tag format specified in the tool descriptions.\n\
         - DO NOT use JSON formatting.\n\
         - DO NOT use special tokens (like `<|tool_calls_section_begin|>`, `<|tool_call_begin|>`, etc.).\n\
         - DO NOT prefix tool names with `functions.`.\n\
         - ALWAYS write tool calls as plain text XML tags, e.g., `<load_tools group=\"fs\">`.\n\
         \n\
         Built-in groups: {group_list}\n\
         \n\
         ### `load_tools` Description\n\
         Loads and displays tool definitions for a specific group of tools.\n\
         \n\
         Format (Attributes - preferred):\n\n\
         ```example\n\
         <load_tools group=\"fs\">\n\
         ```\n\
         \n\
         - Call without a group parameter (e.g., `<load_tools>`) to list all registered tool groups.\n\
         - Specify `group` (e.g., `fs`, `shell`, `web`, `todo`, `ask`, `memory`) to load and register that group's tools.\n\
         - Newly loaded tools become available for use on all subsequent turns.",
    ))
}
