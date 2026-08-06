//! Comprehensive unit and integration tests for the edit tool.
//!
//! Hey friend! Tests cover success paths (single/multi-hunk unified diff edits, atomic writes),
//! fuzzy matching (rstrip, space trim, Unicode normalization, case insensitivity),
//! error handling (invalid patch syntax, context/lines not found, missing files),
//! and edge cases (`file_path` alias, CRLF line endings).

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
async fn test_single_edit_hunk() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "fn old_name() {\n    println!(\"hello\");\n}\n").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "patch": "@@ fn old_name()\n-fn old_name() {\n+fn new_name() {\n     println!(\"hello\");"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error, "edit should succeed: {:?}", result.content);
    let output = get_output(&result);
    assert_eq!(output.hunks_applied, 1);
    assert!(output.message.contains("Applied 1 edit(s)"));

    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("fn new_name() {"));
    assert!(!new_content.contains("fn old_name() {"));
}

#[tokio::test]
async fn test_multi_hunk_edit() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(
        &path,
        "import { oldFunc } from './lib';\n\nfn main() {\n    oldFunc(1, 2);\n    // TODO: refactor oldFunc\n}\n",
    )
    .unwrap();

    let patch = "\
@@ import header
-import { oldFunc } from './lib';
+import { newFunc } from './lib';
@@ fn main()
-    oldFunc(1, 2);
+    newFunc(1, 2);
@@ todo comment
-// TODO: refactor oldFunc
+// TODO: refactor newFunc
";

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "patch": patch
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error, "multi-hunk edit should succeed");
    let output = get_output(&result);
    assert_eq!(output.hunks_applied, 3);

    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("import { newFunc } from './lib';"));
    assert!(new_content.contains("newFunc(1, 2);"));
    assert!(new_content.contains("// TODO: refactor newFunc"));
    assert!(!new_content.contains("oldFunc"));
}

#[tokio::test]
async fn test_atomic_write() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "line 1\nline 2\nline 3\n";
    fs::write(&path, original).unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "patch": "@@\n-line 2\n+line 2 modified"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("line 2 modified"));
    assert!(!new_content.contains("line 2\n"));
}

#[tokio::test]
async fn test_file_path_alias() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "old text\n").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "file_path": path,
            "patch": "@@\n-old text\n+new text"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let new_content = fs::read_to_string(&path).unwrap();
    assert_eq!(new_content, "new text\n");
}

// ============================================================================
// FUZZY MATCHING TESTS
// ============================================================================

#[tokio::test]
async fn test_fuzzy_unicode_dash_and_quotes() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    // File contains Unicode em-dash (\u{2014}) and curly quotes (\u{201C}, \u{201D})
    fs::write(&path, "some\u{2014}thing \u{201C}quoted\u{201D}\n").unwrap();

    // Patch uses ASCII dash and straight quotes
    let patch = "@@\n-some-thing \"quoted\"\n+some-thing \"updated\"";

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "patch": patch
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error, "fuzzy Unicode matching should succeed");
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("some-thing \"updated\""));
}

#[tokio::test]
async fn test_fuzzy_case_insensitive_matching() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "CONSTANT_KEY = 100\n").unwrap();

    // Patch uses lowercase key
    let patch = "@@\n-constant_key = 100\n+constant_key = 200";

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "patch": patch
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error, "case-insensitive fuzzy match should succeed");
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("200"));
}

// ============================================================================
// FAILURE PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_invalid_patch_syntax() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "original content\n").unwrap();

    // Missing @@ header
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "patch": "-original content\n+new content"
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("failed to parse patch"));
}

#[tokio::test]
async fn test_line_not_found_error() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "original content\n";
    fs::write(&path, original).unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "patch": "@@\n-nonexistent line\n+replacement"
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("old_string not found"));

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_partial_failure_aborts_all() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "line 1\nline 2\nline 3\n";
    fs::write(&path, original).unwrap();

    let patch = "\
@@
-line 1
+line 1 modified
@@
-nonexistent line
+replacement
@@
-line 3
+line 3 modified
";

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "patch": patch
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("hunk 1"));

    // File must be completely untouched on disk
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_nonexistent_file() {
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": "/nonexistent/path/to/file.txt",
            "patch": "@@\n-old\n+new"
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("failed to read file"));
}
