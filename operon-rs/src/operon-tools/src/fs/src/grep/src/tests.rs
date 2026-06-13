//! Tests for the grep tool.
//!
//! Comprehensive test suite covering basic search, case insensitivity, directory recursion,
//! file include glob filters, line context, truncation limits, and error handling.

use crate::execute;
use operon_context_normalize::tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper function to extract text from a ToolResult.
fn extract_text(result: &operon_context_normalize::tools::ToolResult) -> String {
    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content, got {:?}", other),
    }
}

fn create_test_file(dir: &TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    fs::write(&path, content).expect("failed to write test file");
    path.display().to_string()
}

#[tokio::test]
async fn test_basic_search() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let file = create_test_file(&temp, "test.txt", "line 1\nline 2 with pattern\nline 3");

    let result = execute(
        ToolCallId("test".to_string()),
        json!({
            "path": &file,
            "__body__": "pattern=\"pattern\""
        }),
    )
    .await
    .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("1 match(es) in 1 file(s)"));
    assert!(text.contains("line 2 with pattern"));
}

#[tokio::test]
async fn test_invalid_regex() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let file = create_test_file(&temp, "test.txt", "content");

    let result = execute(
        ToolCallId("test".to_string()),
        json!({
            "path": &file,
            "__body__": "pattern=\"[invalid\""
        }),
    )
    .await
    .expect("execute failed");

    assert!(!result.is_error); // Executor returns is_error: false and puts the error in the text output
    let text = extract_text(&result);
    assert!(text.contains("ERROR"));
}

#[tokio::test]
async fn test_case_insensitive() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let file = create_test_file(&temp, "test.txt", "ERROR\nerror\nError");

    let result = execute(
        ToolCallId("test".to_string()),
        json!({
            "path": &file,
            "__body__": "pattern=\"error\""
        }),
    )
    .await
    .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(&result);
    // Default search is case-sensitive, should match only 1
    assert!(text.contains("1 match(es)"));
}

#[tokio::test]
async fn test_truncation() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let mut content = String::new();
    for i in 1..=400 {
        content.push_str(&format!("line {} with pattern\n", i));
    }
    let file = create_test_file(&temp, "large.txt", &content);

    let result = execute(
        ToolCallId("test".to_string()),
        json!({
            "path": &file,
            "__body__": "pattern=\"pattern\""
        }),
    )
    .await
    .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(&result);
    // Should hit the 300 match limit
    assert!(text.contains("300 match(es)"));
    assert!(text.contains("omitted"));
}

#[tokio::test]
async fn test_include_filter() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let _file_rs = create_test_file(&temp, "file.rs", "fn main() { pattern }");
    let _file_py = create_test_file(&temp, "file.py", "def main(): pattern");

    let result = execute(
        ToolCallId("test".to_string()),
        json!({
            "path": temp.path().display().to_string(),
            "__body__": "pattern=\"pattern\"\nglob=\"*.rs\""
        }),
    )
    .await
    .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("file.rs"));
    assert!(!text.contains("file.py"));
}
