//! Comprehensive tests for the append tool.
//!
//! Tests cover success paths (appending to existing files, multiple appends, Unicode),
//! failure paths (nonexistent file, directory path, empty content), and edge cases
//! (no trailing newline, leading newline, empty file).

use crate::execute;
use operon_context_normalize::tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::NamedTempFile;

/// Helper to extract text from a ToolResult.
fn extract_text(result: &operon_context_normalize::tools::ToolResult) -> String {
    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content, got {:?}", other),
    }
}

#[tokio::test]
async fn test_basic_append() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let initial_content = "line 1\n";
    fs::write(&path, initial_content).unwrap();

    let append_content = "line 2\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("done"));
    assert!(text.contains(&path));

    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "line 1\nline 2\n");
}

#[tokio::test]
async fn test_multiple_appends() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial\n").unwrap();

    for i in 1..=3 {
        let result = execute(
            ToolCallId(format!("call_{}", i)),
            json!({
                "path": &path,
                "__body__": format!("append {}\n", i)
            }),
        )
        .await
        .unwrap();
        assert!(!result.is_error);
    }

    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "initial\nappend 1\nappend 2\nappend 3\n");
}

#[tokio::test]
async fn test_append_no_trailing_newline() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "existing").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "more"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "existingmore");
}

#[tokio::test]
async fn test_append_with_leading_newline() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "line 1").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "\nline 2"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "line 1\nline 2");
}

#[tokio::test]
async fn test_append_to_empty_file() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "content"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "content");
}

#[tokio::test]
async fn test_file_not_found() {
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": "/tmp/does_not_exist_xyz_operon_test/file.txt",
            "__body__": "content"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("ERROR"));
    assert!(text.contains("does not exist") || text.contains("No such file"));
}

#[tokio::test]
async fn test_path_is_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": dir_path,
            "__body__": "content"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("ERROR"));
}

#[tokio::test]
async fn test_empty_content_error() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "__body__": ""
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("ERROR"));
    assert!(text.contains("empty"));
}
