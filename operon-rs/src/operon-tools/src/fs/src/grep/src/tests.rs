//! Tests for the grep tool.

use super::*;
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper to create a test file with given content.
fn create_test_file(dir: &TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    fs::write(&path, content).expect("failed to write test file");
    path.display().to_string()
}

fn extract_text(result: operon_context_normalize_tools::ToolResult) -> String {
    match result.content {
        ToolContent::Text(t) => t,
        other => panic!("expected Text content, got {:?}", other),
    }
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

    let text = extract_text(result);
    assert!(text.contains("2: line 2 with pattern"));
    assert!(text.contains("Showing 1 match(es) across 1 file(s)."));
}

#[tokio::test]
async fn test_single_path_string_alias() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let file = create_test_file(&temp, "test.txt", "hello world");

    let args = json!({
        "pattern": "hello",
        "path": file
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("1: hello world"));
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
    let text = extract_text(result);
    assert!(text.contains("1: ERROR"));
    assert!(text.contains("2: error"));
    assert!(text.contains("3: Error"));
    assert!(text.contains("Showing 3 match(es) across 1 file(s)."));
}

#[tokio::test]
async fn test_context_separator() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let file = create_test_file(
        &temp,
        "test.txt",
        "match 1\nline 2\nline 3\nline 4\nline 5\nline 6\nmatch 2",
    );

    let args = json!({
        "pattern": "match",
        "paths": [file],
        "context_lines": 0
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("1: match 1\n---\n7: match 2"));
}

#[tokio::test]
async fn test_missing_required_fields() {
    // Missing "paths" field
    let args = json!({
        "pattern": "test"
    });

    let result = execute(ToolCallId("test".to_string()), args).await;
    assert!(result.is_err() || result.unwrap().is_error);

    // Missing "pattern" field
    let args = json!({
        "paths": ["test.txt"]
    });

    let result = execute(ToolCallId("test".to_string()), args).await;
    assert!(result.is_err() || result.unwrap().is_error);
}

#[tokio::test]
async fn test_truncation() {
    let temp = TempDir::new().expect("failed to create temp dir");

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
    let text = extract_text(result);
    assert!(text.contains("300 match(es)"));
}

#[tokio::test]
async fn test_truncation_multiple_files() {
    let temp = TempDir::new().expect("failed to create temp dir");

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
    let text = extract_text(result);
    assert!(text.contains("300 match(es)"));
}

#[tokio::test]
async fn test_include_filter() {
    let temp = TempDir::new().expect("failed to create temp dir");

    let _file_rs = create_test_file(&temp, "file.rs", "fn main() { pattern }");
    let _file_py = create_test_file(&temp, "file.py", "def main(): pattern");

    let args = json!({
        "pattern": "pattern",
        "paths": [temp.path().display().to_string()],
        "include": "*.rs"
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("file.rs"));
    assert!(!text.contains("file.py"));
    assert!(text.contains("Showing 1 match(es) across 1 file(s)."));
}

#[tokio::test]
async fn test_grep_defensive_aliases_and_stringified_paths() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let _file_rs = create_test_file(&temp, "file.rs", "let magic_token = 42;");

    // Test with query alias, stringified JSON paths array, context string
    let args = json!({
        "query": "magic_token",
        "path": format!("[\"{}\"]", temp.path().display().to_string().replace('\\', "\\\\")),
        "context": "3"
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("magic_token"));
    assert!(text.contains("1 match(es)"));
}

#[tokio::test]
async fn test_grep_relative_path_rejected() {
    let args = json!({
        "pattern": "some_pattern",
        "paths": ["relative/path/src"]
    });

    let result = execute(ToolCallId("rel_call".to_string()), args)
        .await
        .expect("execute failed");

    assert!(result.is_error);
    let text = extract_text(result);
    assert!(text.contains("Path must be an absolute path"));
}
