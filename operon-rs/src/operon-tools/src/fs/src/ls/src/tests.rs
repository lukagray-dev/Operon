//! Tests for the ls tool.
//!
//! Comprehensive test suite covering basic listing, exclusion patterns, error cases,
//! hidden files, truncation, and sorting.

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

#[tokio::test]
async fn test_basic_listing() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
    fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
    fs::create_dir(temp_dir.path().join("subdir")).unwrap();

    let path = temp_dir.path().to_string_lossy().to_string();
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({ "path": &path }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    let lines: Vec<&str> = text.lines().collect();

    // First line is path header
    assert!(lines[0].contains(&path) || path.contains(lines[0]));

    // Check directory listing contains the items
    assert!(text.contains("[DIR]  subdir"));
    assert!(text.contains("[FILE] file1.txt"));
    assert!(text.contains("[FILE] file2.txt"));
}

#[tokio::test]
async fn test_ignore_patterns() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join("Cargo.lock"), "").unwrap();
    fs::create_dir(temp_dir.path().join("node_modules")).unwrap();
    fs::create_dir(temp_dir.path().join("src")).unwrap();
    fs::write(temp_dir.path().join("main.rs"), "").unwrap();

    let path = temp_dir.path().to_string_lossy().to_string();
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "ignore": "*.lock\nnode_modules"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(!text.contains("Cargo.lock"));
    assert!(!text.contains("node_modules"));
    assert!(text.contains("[DIR]  src"));
    assert!(text.contains("[FILE] main.rs"));
}

#[tokio::test]
async fn test_file_path_error() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, "content").unwrap();

    let path = file_path.to_string_lossy().to_string();
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({ "path": &path }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("ERROR"));
    assert!(text.contains("file, not a directory"));
}

#[tokio::test]
async fn test_nonexistent_path() {
    let path = "/nonexistent/path/that/does/not/exist";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({ "path": path }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("ERROR"));
}

#[tokio::test]
async fn test_hidden_files_included() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join(".env"), "secret").unwrap();
    fs::write(temp_dir.path().join("normal.txt"), "content").unwrap();

    let path = temp_dir.path().to_string_lossy().to_string();
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({ "path": &path }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("[FILE] .env"));
    assert!(text.contains("[FILE] normal.txt"));
}

#[tokio::test]
async fn test_sorting_case_insensitive() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::create_dir(temp_dir.path().join("Zebra")).unwrap();
    fs::create_dir(temp_dir.path().join("apple")).unwrap();
    fs::write(temp_dir.path().join("Zulu.txt"), "").unwrap();
    fs::write(temp_dir.path().join("alpha.txt"), "").unwrap();

    let path = temp_dir.path().to_string_lossy().to_string();
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({ "path": &path }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    let lines: Vec<&str> = text.lines().collect();

    // Lines are: header, Zebra/apple, alpha.txt/Zulu.txt
    // Directories must come first: apple, Zebra, then alpha.txt, Zulu.txt
    let idx_apple = lines.iter().position(|l| l.contains("apple")).unwrap();
    let idx_zebra = lines.iter().position(|l| l.contains("Zebra")).unwrap();
    let idx_alpha = lines.iter().position(|l| l.contains("alpha.txt")).unwrap();
    let idx_zulu = lines.iter().position(|l| l.contains("Zulu.txt")).unwrap();

    assert!(idx_apple < idx_zebra);
    assert!(idx_zebra < idx_alpha);
    assert!(idx_alpha < idx_zulu);
}

#[tokio::test]
async fn test_attribute_based_listing() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join("Cargo.lock"), "").unwrap();
    fs::create_dir(temp_dir.path().join("node_modules")).unwrap();
    fs::create_dir(temp_dir.path().join("src")).unwrap();
    fs::write(temp_dir.path().join("main.rs"), "").unwrap();

    let path = temp_dir.path().to_string_lossy().to_string();
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "paths": &path,
            "ignore": "*.lock\nnode_modules"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(!text.contains("Cargo.lock"));
    assert!(!text.contains("node_modules"));
    assert!(text.contains("[DIR]  src"));
    assert!(text.contains("[FILE] main.rs"));
}

