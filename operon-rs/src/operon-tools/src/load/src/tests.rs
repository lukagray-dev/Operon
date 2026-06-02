//! Tests for the load_tools tool.

#[cfg(test)]
mod tests {
    use crate::{execute_list_groups, execute_with_defs, GroupListOutput, GroupLoadOutput};
    use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolDefinition};
    use serde_json::json;

    /// Test that execute_with_defs returns tools correctly.
    #[test]
    fn execute_with_defs_returns_tools() {
        let tool_def = ToolDefinition {
            name: "read".to_string(),
            description: "Reads files.".to_string(),
            parameters: json!({ "type": "object" }),
        };
        let defs = vec![&tool_def];

        let result = execute_with_defs(ToolCallId("test".to_string()), "fs", defs);

        assert!(!result.is_error);
        let output: GroupLoadOutput = match result.content {
            ToolContent::Json(v) => serde_json::from_value(v).unwrap(),
            _ => panic!("expected Json"),
        };
        assert_eq!(output.group, "fs");
        assert_eq!(output.tool_count, 1);
        assert_eq!(output.tools[0].name, "read");
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
        let output: GroupListOutput = match result.content {
            ToolContent::Json(v) => serde_json::from_value(v).unwrap(),
            _ => panic!("expected Json"),
        };
        assert!(output.available_groups.contains(&"fs".to_string()));
        assert!(output.available_groups.contains(&"shell".to_string()));
        assert!(output.message.contains("fs, shell"));
    }

    /// Test that execute_list_groups handles empty group list.
    #[test]
    fn execute_list_groups_empty() {
        let result = execute_list_groups(ToolCallId("test".to_string()), vec![]);
        assert!(!result.is_error);
        let output: GroupListOutput = match result.content {
            ToolContent::Json(v) => serde_json::from_value(v).unwrap(),
            _ => panic!("expected Json"),
        };
        assert!(output.available_groups.is_empty());
    }
}
