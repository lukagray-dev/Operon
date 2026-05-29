/// Tests for the grep tool.
///
/// This module contains unit and integration tests for the grep tool implementation.
/// Tests cover argument parsing, pattern matching, directory walking, glob filtering,
/// context lines, error handling, and limit enforcement.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::output::GrepOutput;
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

        // Parse the ToolContent as JSON and verify match counts.
        let content_value = match &result.content {
            operon_context_normalize_tools::ToolContent::Json(v) => v.clone(),
            other => panic!("expected JSON content, got {:?}", other),
        };
        let output: GrepOutput = serde_json::from_value(content_value)
            .expect("failed to deserialize GrepOutput");

        assert_eq!(output.total_matches, 1, "expected exactly 1 match");
        assert_eq!(output.files_with_matches, 1);
        assert!(!output.truncated);
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.files[0].match_count, 1);
        assert_eq!(output.files[0].matches[0].line_no, 2);
        assert!(output.files[0].matches[0].is_match);
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

        // Parse the ToolContent as JSON and verify all three lines matched.
        let content_value = match &result.content {
            operon_context_normalize_tools::ToolContent::Json(v) => v.clone(),
            other => panic!("expected JSON content, got {:?}", other),
        };
        let output: GrepOutput = serde_json::from_value(content_value)
            .expect("failed to deserialize GrepOutput");

        assert_eq!(output.total_matches, 3, "expected exactly 3 matches");
        assert_eq!(output.files_with_matches, 1);
        assert!(!output.truncated);
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

    #[tokio::test]
    async fn test_truncation() {
        let temp = TempDir::new().expect("failed to create temp dir");
        
        // Create a file with 400 lines, each containing the pattern.
        let mut content = String::new();
        for i in 1..=400 {
            content.push_str(&format!("line {} with pattern\n", i));
        }
        let file = create_test_file(&temp, "large.txt", &content);

        let args = json!({
            "pattern": "pattern",
            "paths": [file]
        });

        let result = execute(ToolCallId("test".to_string()), args)
            .await
            .expect("execute failed");

        assert!(!result.is_error);

        // Parse the ToolContent as JSON and verify truncation.
        let content_value = match &result.content {
            operon_context_normalize_tools::ToolContent::Json(v) => v.clone(),
            other => panic!("expected JSON content, got {:?}", other),
        };
        let output: GrepOutput = serde_json::from_value(content_value)
            .expect("failed to deserialize GrepOutput");

        // Should hit the 300 match limit and stop.
        assert_eq!(output.total_matches, 300, "expected exactly 300 matches (limit)");
        assert!(!output.truncated, "truncated should be false when searching a single file");
        assert_eq!(output.files_with_matches, 1);
    }

    #[tokio::test]
    async fn test_truncation_multiple_files() {
        let temp = TempDir::new().expect("failed to create temp dir");
        
        // Create 3 files, each with 150 lines containing the pattern.
        // File1: 150 matches (total: 150)
        // File2: 150 matches (total: 300) - hits limit exactly
        // File3: would have 150 more, but should be skipped entirely
        let mut content = String::new();
        for i in 1..=150 {
            content.push_str(&format!("line {} with pattern\n", i));
        }
        let file1 = create_test_file(&temp, "file1.txt", &content);
        let file2 = create_test_file(&temp, "file2.txt", &content);
        let file3 = create_test_file(&temp, "file3.txt", &content);

        let args = json!({
            "pattern": "pattern",
            "paths": [file1, file2, file3]
        });

        let result = execute(ToolCallId("test".to_string()), args)
            .await
            .expect("execute failed");

        assert!(!result.is_error);

        // Parse the ToolContent as JSON and verify truncation.
        let content_value = match &result.content {
            operon_context_normalize_tools::ToolContent::Json(v) => v.clone(),
            other => panic!("expected JSON content, got {:?}", other),
        };
        let output: GrepOutput = serde_json::from_value(content_value)
            .expect("failed to deserialize GrepOutput");

        // After file2 completes with 300 total matches, we check before file3.
        // Since 300 >= 300, we break and set truncated = true.
        assert!(output.truncated, "truncated should be true when skipping files");
        assert_eq!(output.total_matches, 300, "expected exactly 300 matches (limit)");
        // Only file1 and file2 should be in results since file3 was skipped.
        assert_eq!(output.files_with_matches, 2);
        assert_eq!(output.files.len(), 2);
    }

    #[tokio::test]
    async fn test_include_filter() {
        let temp = TempDir::new().expect("failed to create temp dir");
        
        // Create two files with the same pattern, but different extensions.
        let _file_rs = create_test_file(&temp, "file.rs", "fn main() { pattern }");
        let _file_py = create_test_file(&temp, "file.py", "def main(): pattern");

        // Search the directory with a filter for only .rs files.
        let args = json!({
            "pattern": "pattern",
            "paths": [temp.path().display().to_string()],
            "include": "*.rs"
        });

        let result = execute(ToolCallId("test".to_string()), args)
            .await
            .expect("execute failed");

        assert!(!result.is_error);

        // Parse the ToolContent as JSON and verify only .rs file appears.
        let content_value = match &result.content {
            operon_context_normalize_tools::ToolContent::Json(v) => v.clone(),
            other => panic!("expected JSON content, got {:?}", other),
        };
        let output: GrepOutput = serde_json::from_value(content_value)
            .expect("failed to deserialize GrepOutput");

        assert_eq!(output.total_matches, 1, "expected exactly 1 match");
        assert_eq!(output.files_with_matches, 1);
        assert_eq!(output.files.len(), 1);
        // Verify the matched file is the .rs file.
        assert!(output.files[0].path.ends_with("file.rs"), 
                "expected file.rs, got {}", output.files[0].path);
    }
}
