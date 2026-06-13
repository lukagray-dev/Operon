//! Tests for the load_tools tool.

#[cfg(test)]
mod tests {
    use crate::{execute_list_groups, execute_with_defs};
    use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolDefinition};

    /// Test that execute_with_defs returns tools correctly as plain-text markdown.
    #[test]
    fn execute_with_defs_returns_tools_markdown() {
        let tool_def = ToolDefinition {
            name: "read".to_string(),
            description: "Reads files.".to_string(),
        };
        let defs = vec![&tool_def];

        let result = execute_with_defs(ToolCallId("test".to_string()), "fs", defs);

        assert!(!result.is_error);
        let text = match result.content {
            ToolContent::Text(t) => t,
            _ => panic!("expected Text"),
        };
        assert!(text.contains("Loaded 1 tool(s) from group 'fs':"));
        assert!(text.contains("## read"));
        assert!(text.contains("Reads files."));
    }

    /// Test that execute_with_defs returns tools with body calling syntax correctly.
    #[test]
    fn execute_with_defs_returns_body_tools_markdown() {
        let tool_def = ToolDefinition {
            name: "write".to_string(),
            description: "Writes a file.".to_string(),
        };
        let defs = vec![&tool_def];

        let result = execute_with_defs(ToolCallId("test".to_string()), "fs", defs);

        assert!(!result.is_error);
        let text = match result.content {
            ToolContent::Text(t) => t,
            _ => panic!("expected Text"),
        };
        assert!(text.contains("## write"));
        assert!(text.contains("Writes a file."));
    }

    /// Test that execute_with_defs returns error for empty group.
    #[test]
    fn execute_with_defs_empty_returns_error() {
        let result = execute_with_defs(ToolCallId("test".to_string()), "nonexistent", vec![]);
        assert!(result.is_error);
        match result.content {
            ToolContent::Text(msg) => {
                assert!(msg.contains("unknown group"));
                assert!(msg.contains("nonexistent"));
            }
            _ => panic!("expected Text error"),
        }
    }

    /// Test that execute_list_groups returns all groups.
    #[test]
    fn execute_list_groups_returns_all() {
        let result = execute_list_groups(
            ToolCallId("test".to_string()),
            vec!["fs".to_string(), "shell".to_string()],
        );
        assert!(!result.is_error);
        let text = match result.content {
            ToolContent::Text(t) => t,
            _ => panic!("expected Text"),
        };
        assert!(text.contains("Available groups: fs, shell"));
        assert!(text.contains("Call load_tools with a group name"));
    }
}
