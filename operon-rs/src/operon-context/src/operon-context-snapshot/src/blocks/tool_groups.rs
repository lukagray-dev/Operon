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
         Tools are not loaded by default. Use the `load_tools` tool to load a group's \
         tool definitions before calling any tool in it.\n\
         \n\
         load_tools(group?: string) — Pass `group` to get tool names, descriptions, and \
         schemas for that group. Omit `group` to list all available groups.\n\
         \n\
         Built-in groups: {group_list}\n\
         \n\
         For installed extensions (OHub): use `mcp_load` to discover and load \
         extension tools.",
    ))
}
