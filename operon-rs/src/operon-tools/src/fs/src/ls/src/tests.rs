//! Tests for the ls tool.

use crate::execute;
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn extract_text(result: operon_context_normalize_tools::ToolResult) -> String {
    match result.content {
        ToolContent::Text(t) => t,
        other => panic!("expected Text content, got {:?}", other),
    }
}

#[tokio::test]
async fn test_basic_listing() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join("file1.txt"), "content1").expect("failed write");
    fs::write(temp_dir.path().join("file2.txt"), "content2").expect("failed write");
    fs::create_dir(temp_dir.path().join("subdir")).expect("failed mkdir");

    let args = json!({
        "path": temp_dir.path().to_str().unwrap()
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("[DIR]  subdir/"));
    assert!(text.contains("[FILE] file1.txt (8 B)"));
    assert!(text.contains("[FILE] file2.txt (8 B)"));
}

#[tokio::test]
async fn test_ignore_patterns() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join("Cargo.lock"), "").expect("failed write");
    fs::create_dir(temp_dir.path().join("node_modules")).expect("failed mkdir");
    fs::create_dir(temp_dir.path().join("src")).expect("failed mkdir");

    let args = json!({
        "path": temp_dir.path().to_str().unwrap(),
        "ignore": ["*.lock", "node_modules"]
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("[DIR]  src/"));
    assert!(!text.contains("Cargo.lock"));
    assert!(!text.contains("node_modules"));
}

#[tokio::test]
async fn test_dir_alias_and_default() {
    let args = json!({});
    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("==="));
}

#[tokio::test]
async fn test_file_path_error() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, "content").expect("failed write");

    let args = json!({
        "path": file_path.to_str().unwrap()
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("Error:"));
}

#[tokio::test]
async fn test_nonexistent_path() {
    let args = json!({
        "path": "/nonexistent/path/that/does/not/exist"
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("Error:"));
}

