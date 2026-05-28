/// Tests for the grep tool.
///
/// This module contains unit and integration tests for the grep tool implementation.
/// Tests cover argument parsing, pattern matching, directory walking, glob filtering,
/// context lines, error handling, and limit enforcement.

#[cfg(test)]
mod tests {
    use super::super::*;
    use operon_context_normalize_tools::ToolCallId;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test file with given content.
    fn create_test_file(dir: &TempDir, name: &str, content: &str) -> String {
        let path = dir.path().join(name);
        fs::write(&path, content).expect("failed to write test file");
        path.display().to_string()
    }

    #[tokio::test]
    async fn test_basic_search() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let file = create_test_file(&temp, "test.txt", "line 1\nline 2 with pattern\nline 3");

        let args = json!({
            "pattern": "pattern",
            "paths": [file]
        });

        let result = execute(ToolCallId("test".to_string()), args)
            .await
            .expect("execute failed");

        assert!(!result.is_error);
        assert_eq!(result.name, "grep");
    }

    #[tokio::test]
    async fn test_invalid_regex() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let file = create_test_file(&temp, "test.txt", "content");

        let args = json!({
            "pattern": "[invalid",  // Unclosed bracket
            "paths": [file]
        });

        let result = execute(ToolCallId("test".to_string()), args)
            .await
            .expect("execute failed");

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_case_insensitive() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let file = create_test_file(&temp, "test.txt", "ERROR\nerror\nError");

        let args = json!({
            "pattern": "error",
            "paths": [file],
            "case_insensitive": true
        });

        let result = execute(ToolCallId("test".to_string()), args)
            .await
            .expect("execute failed");

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_missing_required_fields() {
        // Missing "paths" field
        let args = json!({
            "pattern": "test"
        });

        let result = execute(ToolCallId("test".to_string()), args).await;
        assert!(result.is_err());

        // Missing "pattern" field
        let args = json!({
            "paths": ["test.txt"]
        });

        let result = execute(ToolCallId("test".to_string()), args).await;
        assert!(result.is_err());
    }
}
