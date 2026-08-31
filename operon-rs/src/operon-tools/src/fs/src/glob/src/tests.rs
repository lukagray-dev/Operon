//! Tests for the glob tool.

use super::*;
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper to create a test file in a directory.
fn create_file(dir: &TempDir, rel_path: &str, content: &str) {
    let full_path = dir.path().join(rel_path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full_path, content).unwrap();
}

fn extract_text(result: operon_context_normalize_tools::ToolResult) -> String {
    match result.content {
        ToolContent::Text(t) => t,
        other => panic!("expected Text content, got {:?}", other),
    }
}

#[tokio::test]
async fn test_glob_basic_pattern_matching() {
    let temp = TempDir::new().unwrap();
    create_file(&temp, "src/lib.rs", "// lib");
    create_file(&temp, "src/main.rs", "// main");
    create_file(&temp, "src/util/helper.rs", "// helper");
    create_file(&temp, "README.md", "# Readme");

    let args = json!({
        "pattern": "**/*.rs",
        "path": temp.path().to_str().unwrap()
    });

    let result = execute(ToolCallId("glob_1".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result);
    assert!(text.contains("3 match(es)"));
    assert!(text.contains("src/lib.rs"));
    assert!(text.contains("src/main.rs"));
    assert!(text.contains("src/util/helper.rs"));
    assert!(!text.contains("README.md"));
}

#[tokio::test]
async fn test_glob_simple_flat_extension() {
    let temp = TempDir::new().unwrap();
    create_file(&temp, "Cargo.toml", "[package]");
    create_file(&temp, "Cargo.lock", "# lock");
    create_file(&temp, "src/main.rs", "fn main() {}");

    let args = json!({
        "pattern": "*.toml",
        "path": temp.path().to_str().unwrap()
    });

    let result = execute(ToolCallId("glob_2".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result);
    assert!(text.contains("Cargo.toml"));
    assert!(!text.contains("Cargo.lock"));
    assert!(!text.contains("src/main.rs"));
}

#[tokio::test]
async fn test_glob_truncation_limit() {
    let temp = TempDir::new().unwrap();
    for i in 0..10 {
        create_file(&temp, &format!("item_{}.txt", i), "test");
    }

    let args = json!({
        "pattern": "*.txt",
        "path": temp.path().to_str().unwrap(),
        "max_results": 3
    });

    let result = execute(ToolCallId("glob_3".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result);
    assert!(text.contains("10 match(es), truncated"));
    assert!(text.contains("[Results truncated at 3 matches."));
}

#[tokio::test]
async fn test_glob_relative_path_rejected() {
    let args = json!({
        "pattern": "*.rs",
        "path": "relative/path"
    });

    let result = execute(ToolCallId("glob_4".to_string()), args)
        .await
        .unwrap();
    assert!(result.is_error);
    let text = extract_text(result);
    assert!(text.contains("Path must be an absolute path"));
}

#[tokio::test]
async fn test_glob_nonexistent_directory() {
    let non_existent = std::env::temp_dir().join("operon_non_existent_dir_9999");

    let args = json!({
        "pattern": "*.rs",
        "path": non_existent.to_str().unwrap()
    });

    let result = execute(ToolCallId("glob_5".to_string()), args)
        .await
        .unwrap();
    assert!(result.is_error);
    let text = extract_text(result);
    assert!(text.contains("Base directory does not exist"));
}

#[tokio::test]
async fn test_glob_defensive_aliases() {
    let temp = TempDir::new().unwrap();
    create_file(&temp, "file.json", "{}");

    // Use aliases: `glob` instead of `pattern`, `dir` instead of `path`, `limit` instead of `max_results`
    let args = json!({
        "glob": "*.json",
        "dir": temp.path().to_str().unwrap(),
        "limit": 50
    });

    let result = execute(ToolCallId("glob_6".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result);
    assert!(text.contains("file.json"));
}

