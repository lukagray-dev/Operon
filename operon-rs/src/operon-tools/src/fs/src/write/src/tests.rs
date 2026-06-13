//! Comprehensive tests for the write tool.
//!
//! Tests cover success paths (creating new files, overwriting existing files, atomic writes),
//! failure paths (directories as target), and edge cases (empty content, multiline content).

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
async fn test_create_new_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("new_file.txt");
    let path = file_path.to_string_lossy().to_string();

    assert!(!file_path.exists());

    let content = "Hello, world!";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("created"));
    assert!(text.contains(&path));

    assert!(file_path.exists());
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, content);
}

#[tokio::test]
async fn test_overwrite_existing_file() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial content").unwrap();

    let new_content = "completely new content";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": new_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("overwritten"));
    assert!(text.contains(&path));

    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, new_content);
}

#[tokio::test]
async fn test_atomic_write_no_tmp_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.txt");
    let path = file_path.to_string_lossy().to_string();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "test content"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);

    let entries = fs::read_dir(temp_dir.path()).unwrap();
    for entry in entries {
        let entry = entry.unwrap();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        assert!(
            !file_name_str.contains(".operon_write_tmp_"),
            "temp file should not exist: {}",
            file_name_str
        );
    }
}

#[tokio::test]
async fn test_empty_content() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("empty_file.txt");
    let path = file_path.to_string_lossy().to_string();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": ""
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("created"));

    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, "");
}

#[tokio::test]
async fn test_write_to_directory_fails() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().to_string_lossy().to_string();

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
    let text = extract_text(&result);
    assert!(text.contains("ERROR"));
}

#[tokio::test]
async fn test_multiline_content() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("multiline.txt");
    let path = file_path.to_string_lossy().to_string();

    let content = "line 1\nline 2\nline 3\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, content);
}
