//! Comprehensive tests for the edit tool.
//!
//! Tests cover success paths (single and multi-hunk edits, atomic writes),
//! failure paths (zero/multiple matches, identical strings, missing files),
//! and edge cases (partial failure aborts all, file_path alias).

use crate::{execute, EditOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::NamedTempFile;

/// Helper to extract error text from a ToolResult.
fn get_error_text(result: &operon_context_normalize_tools::ToolResult) -> String {
    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content for error, got {:?}", other),
    }
}

/// Helper to extract and deserialize EditOutput from a ToolResult.
fn get_output(result: &operon_context_normalize_tools::ToolResult) -> EditOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize EditOutput")
        }
        other => panic!("expected Json content for success, got {:?}", other),
    }
}

// ============================================================================
// SUCCESS PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_single_edit() {
    // Create a temp file with known content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "fn old_name() {\n    println!(\"hello\");\n}\n").unwrap();

    // Execute a single edit.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "fn old_name() {",
                    "new_string": "fn new_name() {"
                }
            ]
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error, "edit should succeed");
    let output = get_output(&result);
    assert_eq!(output.hunks_applied, 1);
    assert!(output.message.contains("Applied 1 edit(s)"));

    // Verify file content changed.
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("fn new_name() {"));
    assert!(!new_content.contains("fn old_name() {"));
}

#[tokio::test]
async fn test_multi_hunk_edit() {
    // Create a temp file with three distinct regions.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(
        &path,
        "import { oldFunc } from './lib';\n\nfn main() {\n    oldFunc(1, 2);\n    // TODO: refactor oldFunc\n}\n",
    )
    .unwrap();

    // Execute three hunks.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "import { oldFunc } from './lib';",
                    "new_string": "import { newFunc } from './lib';"
                },
                {
                    "old_string": "oldFunc(1, 2);",
                    "new_string": "newFunc(1, 2);"
                },
                {
                    "old_string": "// TODO: refactor oldFunc",
                    "new_string": "// TODO: refactor newFunc"
                }
            ]
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.hunks_applied, 3);

    // Verify all three regions changed.
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("import { newFunc } from './lib';"));
    assert!(new_content.contains("newFunc(1, 2);"));
    assert!(new_content.contains("// TODO: refactor newFunc"));
    assert!(!new_content.contains("oldFunc"));
}

#[tokio::test]
async fn test_atomic_write() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "line 1\nline 2\nline 3\n";
    fs::write(&path, original).unwrap();

    // Execute an edit.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "line 2",
                    "new_string": "line 2 modified"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);

    // Verify the file was written.
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("line 2 modified"));
    assert!(!new_content.contains("line 2\n"));
}

#[tokio::test]
async fn test_file_path_alias() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "old text").unwrap();

    // Use "file_path" instead of "path".
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "file_path": path,
            "edits": [
                {
                    "old_string": "old text",
                    "new_string": "new text"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let new_content = fs::read_to_string(&path).unwrap();
    assert_eq!(new_content, "new text");
}

// ============================================================================
// FAILURE PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_zero_match_error() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "original content";
    fs::write(&path, original).unwrap();

    // Try to edit with a non-existent old_string.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "nonexistent",
                    "new_string": "replacement"
                }
            ]
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("not found"));
    assert!(error.contains("hunk 0"));

    // Verify file unchanged.
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_multiple_match_error() {
    // Create a temp file with old_string appearing twice.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "foo bar\nfoo baz\n";
    fs::write(&path, original).unwrap();

    // Try to edit with an ambiguous old_string.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "foo",
                    "new_string": "bar"
                }
            ]
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("matched 2 times"));
    assert!(error.contains("ambiguous"));

    // Verify file unchanged.
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_identical_strings_error() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    // Try to edit with identical old_string and new_string.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "content",
                    "new_string": "content"
                }
            ]
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("identical"));
}

#[tokio::test]
async fn test_partial_failure_aborts_all() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "line 1\nline 2\nline 3\n";
    fs::write(&path, original).unwrap();

    // Try three hunks where hunk 1 (index 1) has zero matches.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "line 1",
                    "new_string": "line 1 modified"
                },
                {
                    "old_string": "nonexistent",
                    "new_string": "replacement"
                },
                {
                    "old_string": "line 3",
                    "new_string": "line 3 modified"
                }
            ]
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("hunk 1"));

    // Verify file unchanged (hunk 0 was applied in memory but never written).
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_nonexistent_file() {
    // Try to edit a file that doesn't exist.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": "/nonexistent/path/to/file.txt",
            "edits": [
                {
                    "old_string": "old",
                    "new_string": "new"
                }
            ]
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("failed to read file"));
}

#[tokio::test]
async fn test_empty_edits_array() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    // Try to edit with empty edits array.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": []
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("at least one hunk"));
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[tokio::test]
async fn test_hunk_order_matters() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "foo bar baz").unwrap();

    // Apply two hunks where hunk 1 depends on hunk 0's result.
    // Hunk 0: replace "foo" with "bar"
    // Hunk 1: replace "bar bar" with "bar baz" (this matches after hunk 0)
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "foo bar",
                    "new_string": "bar bar"
                },
                {
                    "old_string": "bar bar baz",
                    "new_string": "bar baz baz"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let new_content = fs::read_to_string(&path).unwrap();
    assert_eq!(new_content, "bar baz baz");
}

#[tokio::test]
async fn test_multiline_edit() {
    // Create a temp file with multiline content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "fn old_func() {\n    println!(\"hello\");\n    println!(\"world\");\n}\n";
    fs::write(&path, original).unwrap();

    // Replace a multiline block.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "fn old_func() {\n    println!(\"hello\");\n    println!(\"world\");\n}",
                    "new_string": "fn new_func() {\n    println!(\"goodbye\");\n}"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("fn new_func() {"));
    assert!(new_content.contains("println!(\"goodbye\");"));
}

#[tokio::test]
async fn test_whitespace_exactness() {
    // Create a temp file with specific whitespace (4 spaces at start of line).
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "    indented line\n").unwrap();

    // Verify the file content is what we expect.
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "    indented line\n", "file should have 4 spaces");

    // Try to match with a string that has tabs instead of spaces — should fail.
    // This tests that whitespace is exact (tabs vs spaces).
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "edits": [
                {
                    "old_string": "\t\tindented line\n",
                    "new_string": "\t\tmodified line\n"
                }
            ]
        }),
    )
    .await
    .unwrap();

    // This should fail because the file has spaces, not tabs.
    assert!(
        result.is_error,
        "edit with tabs instead of spaces should fail"
    );
    let error = get_error_text(&result);
    assert!(error.contains("not found"));

    // Now match with correct whitespace (4 spaces) — should succeed.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "    indented line\n",
                    "new_string": "    modified line\n"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(
        !result.is_error,
        "edit with correct whitespace should succeed"
    );
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("    modified line"));
}

#[tokio::test]
async fn test_empty_file() {
    // Create an empty temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "").unwrap();

    // Try to edit an empty file.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "anything",
                    "new_string": "something"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("not found"));
}

#[tokio::test]
async fn test_file_with_no_trailing_newline() {
    // Create a temp file without trailing newline.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "no trailing newline").unwrap();

    // Edit it.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "no trailing newline",
                    "new_string": "has trailing newline\n"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let new_content = fs::read_to_string(&path).unwrap();
    assert_eq!(new_content, "has trailing newline\n");
}
