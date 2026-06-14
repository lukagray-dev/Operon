//! Comprehensive tests for the delete tool.
//!
//! Tests cover success paths (deleting files and directories, trash vs permanent),
//! failure paths (nonexistent paths), and edge cases (nested directories).

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
async fn test_trash_file() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "permanent": "false"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("moved to trash"));
    assert!(text.contains("file"));
    assert!(!std::path::Path::new(&path).exists());
}

#[tokio::test]
async fn test_trash_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();
    let file_path = std::path::Path::new(&dir_path).join("file.txt");
    fs::write(&file_path, "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &dir_path,
            "permanent": "false"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("moved to trash"));
    assert!(text.contains("dir"));
    assert!(!std::path::Path::new(&dir_path).exists());
}

#[tokio::test]
async fn test_default_permanent_is_false() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("moved to trash"), "should default to trash");
    assert!(!std::path::Path::new(&path).exists());
}

#[tokio::test]
async fn test_permanent_delete_file() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "permanent": "true"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("permanently deleted"));
    assert!(text.contains("file"));
    assert!(!std::path::Path::new(&path).exists());
}

#[tokio::test]
async fn test_permanent_delete_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();
    let subdir = std::path::Path::new(&dir_path).join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file.txt"), "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &dir_path,
            "permanent": "true"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("permanently deleted"));
    assert!(text.contains("dir"));
    assert!(!std::path::Path::new(&dir_path).exists());
}

#[tokio::test]
async fn test_path_not_found() {
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": "/tmp/does_not_exist_xyz_operon_test/file.txt"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("does not exist"));
}
