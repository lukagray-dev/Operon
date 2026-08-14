//! Comprehensive unit and integration tests for the edit tool.
//!
//! Hey friend! Tests cover:
//! - Success paths (single and multi-hunk edits, atomic writes, ordering)
//! - Fuzzy matching (Unicode punctuation normalization, case insensitivity, whitespace)
//! - Fast-fail validation (empty edits array, identical old/new strings, missing files)
//! - Partial-success execution semantics (successful hunks land on disk, failed hunks reported in structured diagnostics)
//! - Ambiguity detection and order-dependent partial recovery

use crate::{execute, EditOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::NamedTempFile;

/// Helper to extract error text from a ToolResult (when fast-failing).
fn get_text_content(result: &operon_context_normalize_tools::ToolResult) -> String {
    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content, got {:?}", other),
    }
}

/// Helper to extract and deserialize EditOutput from a ToolResult.
fn get_output(result: &operon_context_normalize_tools::ToolResult) -> EditOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize EditOutput")
        }
        other => panic!("expected Json content for EditOutput, got {:?}", other),
    }
}

// ============================================================================
// SUCCESS PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_single_edit() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "fn old_name() {\n    println!(\"hello\");\n}\n").unwrap();

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

    assert!(!result.is_error, "edit should succeed: {:?}", result.content);
    let output = get_output(&result);
    assert_eq!(output.total_hunks, 1);
    assert_eq!(output.hunks_applied, 1);
    assert_eq!(output.hunks_failed, 0);
    assert!(output.failures.is_empty());
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

    assert!(!result.is_error, "multi-hunk edit should succeed");
    let output = get_output(&result);
    assert_eq!(output.total_hunks, 3);
    assert_eq!(output.hunks_applied, 3);
    assert_eq!(output.hunks_failed, 0);

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
    assert_eq!(new_content, "new text\n");
}

#[tokio::test]
async fn test_hunk_order_matters() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "foo bar baz\n").unwrap();

    // Hunk 0 replaces "foo bar" with "bar bar"
    // Hunk 1 replaces "bar bar baz" with "bar baz baz" (depends on hunk 0 having run first)
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
    assert_eq!(new_content, "bar baz baz\n");
}

#[tokio::test]
async fn test_multiline_edit() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "fn old_func() {\n    println!(\"hello\");\n    println!(\"world\");\n}\n";
    fs::write(&path, original).unwrap();

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
async fn test_file_with_no_trailing_newline() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "no trailing newline").unwrap();

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

// ============================================================================
// FUZZY MATCHING TESTS
// ============================================================================

#[tokio::test]
async fn test_fuzzy_unicode_dash_and_quotes() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    // File contains Unicode em-dash (\u{2014}) and curly quotes (\u{201C}, \u{201D})
    fs::write(&path, "some\u{2014}thing \u{201C}quoted\u{201D}\n").unwrap();

    // Model provides ASCII dash and straight quotes
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "some-thing \"quoted\"",
                    "new_string": "some-thing \"updated\""
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error, "fuzzy Unicode matching should succeed: {:?}", result.content);
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("some-thing \"updated\""));
}

#[tokio::test]
async fn test_fuzzy_case_insensitive_matching() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "const MAX_LIMIT: u32 = 100;\n").unwrap();

    // Model provides lowercase string
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "const max_limit: u32 = 100;",
                    "new_string": "const MAX_LIMIT: u32 = 200;"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error, "case-insensitive fuzzy match should succeed");
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("200"));
}

#[tokio::test]
async fn test_whitespace_exactness() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "    indented line\n").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "    indented line",
                    "new_string": "    modified line"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("    modified line"));
}

// ============================================================================
// VALIDATION & FAST-FAIL TESTS
// ============================================================================

#[tokio::test]
async fn test_empty_edits_array() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content\n").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": []
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let error = get_text_content(&result);
    assert!(error.contains("at least one hunk"));
}

#[tokio::test]
async fn test_identical_strings() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content\n").unwrap();

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

    assert!(result.is_error);
    let error = get_text_content(&result);
    assert!(error.contains("identical"));
}

#[tokio::test]
async fn test_nonexistent_file() {
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

    assert!(result.is_error);
    let error = get_text_content(&result);
    assert!(error.contains("failed to read file"));
}

#[tokio::test]
async fn test_empty_file() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "").unwrap();

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
    let output = get_output(&result);
    assert_eq!(output.hunks_applied, 0);
    assert_eq!(output.hunks_failed, 1);
    assert_eq!(output.failures.len(), 1);
    assert_eq!(output.failures[0].hunk_index, 0);
    assert!(output.failures[0].reason.contains("not found in file"));
}

// ============================================================================
// PARTIAL SUCCESS & FAILURE SEMANTICS TESTS
// ============================================================================

#[tokio::test]
async fn test_partial_success_writes_to_disk() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "line 1\nline 2\nline 3\n";
    fs::write(&path, original).unwrap();

    // 3 hunks: Hunk 0 matches, Hunk 1 fails (not found), Hunk 2 matches
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
                    "old_string": "nonexistent text",
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

    // Partial success has is_error = true so the model knows to retry failed hunks
    assert!(result.is_error);
    let output = get_output(&result);
    assert_eq!(output.total_hunks, 3);
    assert_eq!(output.hunks_applied, 2);
    assert_eq!(output.hunks_failed, 1);
    assert_eq!(output.failures.len(), 1);
    assert_eq!(output.failures[0].hunk_index, 1);
    assert_eq!(output.failures[0].old_string, "nonexistent text");
    assert!(output.failures[0].reason.contains("not found in file"));
    assert!(output.message.contains("Partially applied: 2 of 3 edit(s) written"));

    // Crucial check: Hunks 0 and 2 MUST be written to disk!
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("line 1 modified"), "hunk 0 must be written to disk");
    assert!(content.contains("line 3 modified"), "hunk 2 must be written to disk");
    assert!(content.contains("line 2"), "line 2 must remain unchanged");
}

#[tokio::test]
async fn test_partial_success_with_ambiguous_hunk() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "header\nduplicate\nmiddle\nduplicate\nfooter\n";
    fs::write(&path, original).unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "header",
                    "new_string": "HEADER"
                },
                {
                    "old_string": "duplicate",
                    "new_string": "DUPLICATE"
                },
                {
                    "old_string": "footer",
                    "new_string": "FOOTER"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let output = get_output(&result);
    assert_eq!(output.total_hunks, 3);
    assert_eq!(output.hunks_applied, 2);
    assert_eq!(output.hunks_failed, 1);
    assert_eq!(output.failures.len(), 1);
    assert_eq!(output.failures[0].hunk_index, 1);
    assert!(output.failures[0].reason.contains("ambiguous"));
    assert!(output.failures[0].reason.contains("matched 2 times"));

    // File on disk must reflect header and footer changes
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("HEADER"));
    assert!(content.contains("FOOTER"));
    assert!(content.contains("duplicate"));
}

#[tokio::test]
async fn test_all_hunks_fail_no_disk_write() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original = "line 1\nline 2\n";
    fs::write(&path, original).unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "missing 1",
                    "new_string": "replacement 1"
                },
                {
                    "old_string": "missing 2",
                    "new_string": "replacement 2"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let output = get_output(&result);
    assert_eq!(output.total_hunks, 2);
    assert_eq!(output.hunks_applied, 0);
    assert_eq!(output.hunks_failed, 2);
    assert_eq!(output.failures.len(), 2);
    assert!(output.message.contains("Failed to apply any edits"));

    // File on disk must be identical
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_partial_success_order_dependent() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "start -> intermediate\n").unwrap();

    // Hunk 0: transforms "start -> intermediate" into "intermediate -> finished"
    // Hunk 1: nonexistent (fails)
    // Hunk 2: transforms "intermediate -> finished" into "final result" (depends on Hunk 0 having run)
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "edits": [
                {
                    "old_string": "start -> intermediate",
                    "new_string": "intermediate -> finished"
                },
                {
                    "old_string": "nonexistent marker",
                    "new_string": "should fail"
                },
                {
                    "old_string": "intermediate -> finished",
                    "new_string": "final result"
                }
            ]
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let output = get_output(&result);
    assert_eq!(output.hunks_applied, 2);
    assert_eq!(output.hunks_failed, 1);

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "final result\n");
}
