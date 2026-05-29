//! Comprehensive tests for the append tool.
//!
//! Tests cover success paths (appending to existing files, multiple appends, Unicode),
//! failure paths (nonexistent file, directory path, empty content), and edge cases
//! (no trailing newline, byte counting, file preservation).

use crate::{execute, AppendOutput};
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

/// Helper to extract and deserialize AppendOutput from a ToolResult.
fn get_output(result: &operon_context_normalize_tools::ToolResult) -> AppendOutput {
    match &result.content {
        ToolContent::Json(v) => serde_json::from_value(v.clone())
            .expect("failed to deserialize AppendOutput"),
        other => panic!("expected Json content for success, got {:?}", other),
    }
}

// ============================================================================
// SUCCESS PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_basic_append() {
    // Create a temp file with initial content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let initial_content = "line 1\n";
    fs::write(&path, initial_content).unwrap();

    // Append new content.
    let append_content = "line 2\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error, "append should succeed");
    let output = get_output(&result);
    assert_eq!(output.bytes_appended, append_content.len());
    assert_eq!(output.total_bytes, (initial_content.len() + append_content.len()) as u64);

    // Verify file content.
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "line 1\nline 2\n");
}

#[tokio::test]
async fn test_multiple_appends() {
    // Create a temp file with initial content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial\n").unwrap();

    // First append.
    let result1 = execute(
        ToolCallId("call1".to_string()),
        json!({
            "path": path.clone(),
            "content": "first append\n"
        }),
    )
    .await
    .unwrap();

    assert!(!result1.is_error);
    let output1 = get_output(&result1);
    assert_eq!(output1.bytes_appended, "first append\n".len());

    // Second append.
    let result2 = execute(
        ToolCallId("call2".to_string()),
        json!({
            "path": path.clone(),
            "content": "second append\n"
        }),
    )
    .await
    .unwrap();

    assert!(!result2.is_error);
    let output2 = get_output(&result2);
    assert_eq!(output2.bytes_appended, "second append\n".len());

    // Third append.
    let result3 = execute(
        ToolCallId("call3".to_string()),
        json!({
            "path": path,
            "content": "third append\n"
        }),
    )
    .await
    .unwrap();

    assert!(!result3.is_error);
    let output3 = get_output(&result3);
    assert_eq!(output3.bytes_appended, "third append\n".len());

    // Verify all content is present in correct order.
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(
        file_content,
        "initial\nfirst append\nsecond append\nthird append\n"
    );
}

#[tokio::test]
async fn test_append_no_trailing_newline_warning() {
    // Create a file with content but no trailing newline.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "existing").unwrap();

    // Append content without leading newline.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": "more"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);

    // Verify the concatenation behavior — content is appended as-is.
    // No newline is inserted automatically.
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "existingmore");
}

#[tokio::test]
async fn test_append_with_leading_newline() {
    // Create a file with content but no trailing newline.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "line 1").unwrap();

    // Append content with leading newline for proper separation.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": "\nline 2"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);

    // Verify correct separation.
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "line 1\nline 2");
}

#[tokio::test]
async fn test_bytes_appended_unicode() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial\n").unwrap();

    // Append Unicode content where char count != byte count.
    // "héllo" is 5 characters but 6 bytes (é is 2 bytes in UTF-8).
    let append_content = "héllo";
    let expected_bytes = append_content.as_bytes().len();
    assert_eq!(expected_bytes, 6, "héllo should be 6 bytes");

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(
        output.bytes_appended, expected_bytes,
        "bytes_appended should match UTF-8 byte count"
    );
}

#[tokio::test]
async fn test_total_bytes_accurate() {
    // Create a temp file with known content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let initial_content = "initial content";
    fs::write(&path, initial_content).unwrap();

    // Append known content.
    let append_content = " appended";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    let expected_total = (initial_content.len() + append_content.len()) as u64;
    assert_eq!(
        output.total_bytes, expected_total,
        "total_bytes should equal initial + appended"
    );
}

#[tokio::test]
async fn test_append_to_empty_file() {
    // Create an empty file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, b"").unwrap();

    // Append content to the empty file.
    let append_content = "first content";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.total_bytes, append_content.len() as u64);

    // Verify file content.
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, append_content);
}

#[tokio::test]
async fn test_path_echoed_in_output() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "content": " appended"
        }),
    )
    .await
    .unwrap();

    let output = get_output(&result);
    assert_eq!(output.path, path, "path should be echoed back in output");
}

#[tokio::test]
async fn test_message_format() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial").unwrap();

    let append_content = " appended";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "content": append_content
        }),
    )
    .await
    .unwrap();

    let output = get_output(&result);
    assert!(
        output.message.contains("Appended"),
        "message should contain 'Appended'"
    );
    assert!(
        output.message.contains(&path),
        "message should contain the file path"
    );
    assert!(
        output.message.contains("bytes"),
        "message should mention bytes"
    );
}

// ============================================================================
// FAILURE PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_file_not_found() {
    // Try to append to a file that doesn't exist.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": "/tmp/does_not_exist_xyz_operon_test/file.txt",
            "content": "content"
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("does not exist"));
    assert!(error.contains("write tool"));
}

#[tokio::test]
async fn test_path_is_directory() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();

    // Try to append to a directory.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": dir_path,
            "content": "content"
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("directory"));
}

#[tokio::test]
async fn test_empty_content() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial").unwrap();

    // Try to append empty content.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": ""
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("empty"));
}

#[tokio::test]
async fn test_existing_content_preserved_on_success() {
    // Create a temp file with original content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original_content = "original\n";
    fs::write(&path, original_content).unwrap();

    // Append new content.
    let append_content = "added\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);

    // Verify original content is preserved at the start.
    let file_content = fs::read_to_string(&path).unwrap();
    assert!(
        file_content.starts_with(original_content),
        "original content should be preserved"
    );
    assert_eq!(file_content, "original\nadded\n");
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[tokio::test]
async fn test_multiline_append() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "start\n").unwrap();

    // Append multiline content.
    let append_content = "line 1\nline 2\nline 3\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "start\nline 1\nline 2\nline 3\n");
}

#[tokio::test]
async fn test_large_append() {
    // Create a temp file with initial content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial\n").unwrap();

    // Append large content (1 MB).
    let append_content = "x".repeat(1024 * 1024);
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.bytes_appended, 1024 * 1024);

    // Verify file content includes both initial and appended.
    let file_content = fs::read_to_string(&path).unwrap();
    assert!(file_content.starts_with("initial\n"));
    assert_eq!(file_content.len(), "initial\n".len() + 1024 * 1024);
}

#[tokio::test]
async fn test_special_characters_in_append() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "initial\n").unwrap();

    // Append content with special characters.
    let append_content = "Special chars: !@#$%^&*()_+-=[]{}|;:',.<>?/~`\nEmoji: 🎉 🚀 ✨\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&path).unwrap();
    assert!(file_content.contains("Special chars:"));
    assert!(file_content.contains("🎉"));
}

#[tokio::test]
async fn test_append_without_trailing_newline() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "no trailing newline").unwrap();

    // Append content without trailing newline.
    let append_content = " also no newline";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "no trailing newline also no newline");
}

#[tokio::test]
async fn test_append_only_newline() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "line 1").unwrap();

    // Append just a newline.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": "\n"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.bytes_appended, 1);

    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "line 1\n");
}

#[tokio::test]
async fn test_append_whitespace_only() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    // Append whitespace (spaces, tabs, newlines).
    let append_content = "   \t\t\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": append_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, "content   \t\t\n");
}
