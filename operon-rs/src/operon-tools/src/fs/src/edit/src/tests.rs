//! Comprehensive tests for the edit tool.
//!
//! Tests cover success paths (single and multi-hunk edits, atomic writes),
//! failure paths (zero/multiple matches, identical strings, missing files),
//! and edge cases (partial failure aborts all).

use crate::execute;
use operon_context_normalize::tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

/// Helper to extract text from a ToolResult.
fn extract_text(result: &operon_context_normalize::tools::ToolResult) -> String {
    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content, got {:?}", other),
    }
}

#[tokio::test]
async fn test_single_edit() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    let path = file_path.to_string_lossy().to_string();
    fs::write(&path, "fn old_name() {\n    println!(\"hello\");\n}\n").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "@@\n-fn old_name() {\n+fn new_name() {"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("1 hunk(s) applied"));

    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("fn new_name() {"));
    assert!(!new_content.contains("fn old_name() {"));
}

#[tokio::test]
async fn test_multi_hunk_edit() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    let path = file_path.to_string_lossy().to_string();
    fs::write(
        &path,
        "import { oldFunc } from './lib';\n\nfn main() {\n    oldFunc(1, 2);\n    // TODO: refactor oldFunc\n}\n",
    )
    .unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "@@\n-import { oldFunc } from './lib';\n+import { newFunc } from './lib';\n@@\n-    oldFunc(1, 2);\n+    newFunc(1, 2);\n@@\n-    // TODO: refactor oldFunc\n+    // TODO: refactor newFunc"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("3 hunk(s) applied"));

    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("import { newFunc } from './lib';"));
    assert!(new_content.contains("newFunc(1, 2);"));
    assert!(new_content.contains("// TODO: refactor newFunc"));
}

#[tokio::test]
async fn test_zero_match_error() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    let path = file_path.to_string_lossy().to_string();
    let original = "original content";
    fs::write(&path, original).unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "@@\n-nonexistent\n+replacement"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error); // Executor returns Ok with inline error text
    let text = extract_text(&result);
    assert!(text.contains("not found"));

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_multiple_match_error() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    let path = file_path.to_string_lossy().to_string();
    let original = "foo\nfoo\n";
    fs::write(&path, original).unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "@@\n-foo\n+bar"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("matched 2 times") || text.contains("ambiguous"));

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_identical_strings_error() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    let path = file_path.to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "@@\n-content\n+content"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("identical"));
}

#[tokio::test]
async fn test_partial_failure_aborts_all() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    let path = file_path.to_string_lossy().to_string();
    let original = "line 1\nline 2\nline 3\n";
    fs::write(&path, original).unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": &path,
            "__body__": "@@\n-line 1\n+line 1 modified\n@@\n-nonexistent\n+replacement\n@@\n-line 3\n+line 3 modified"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("hunk 2"));

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_nonexistent_file() {
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": "/nonexistent/path/to/file.txt",
            "__body__": "@@\n-old\n+new"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("failed to read file"));
}
