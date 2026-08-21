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
async fn test_dir_alias_with_absolute_path() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join("sample.txt"), "hello").expect("failed write");

    let args = json!({
        "dir": temp_dir.path().to_str().unwrap()
    });
    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("sample.txt"));
}

#[tokio::test]
async fn test_relative_path_rejected() {
    let args_dot = json!({
        "path": "."
    });
    let result_dot = execute(ToolCallId("test_dot".to_string()), args_dot)
        .await
        .expect("execute failed");

    assert!(!result_dot.is_error);
    let text_dot = extract_text(result_dot);
    assert!(text_dot.contains("Error: Path must be an absolute path"));

    let args_rel = json!({
        "path": "src/subdir"
    });
    let result_rel = execute(ToolCallId("test_rel".to_string()), args_rel)
        .await
        .expect("execute failed");

    assert!(!result_rel.is_error);
    let text_rel = extract_text(result_rel);
    assert!(text_rel.contains("Error: Path must be an absolute path"));
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
    assert!(text.contains("path is a file, not a directory"));
}

#[tokio::test]
async fn test_nonexistent_path() {
    #[cfg(windows)]
    let nonexistent = r"C:\nonexistent\path\that\does\not\exist";
    #[cfg(not(windows))]
    let nonexistent = "/nonexistent/path/that/does/not/exist";

    let args = json!({
        "path": nonexistent
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("Error:"));
}

#[tokio::test]
async fn test_ls_aliases_and_patterns() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join("keep.txt"), "keep").expect("failed write");
    fs::write(temp_dir.path().join("ignore.log"), "log").expect("failed write");

    let args = json!({
        "folder": temp_dir.path().to_str().unwrap(),
        "patterns": ["*.log"]
    });

    let result = execute(ToolCallId("test".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);
    let text = extract_text(result);
    assert!(text.contains("keep.txt"));
    assert!(!text.contains("ignore.log"));
}

