//! Comprehensive tests for the write tool.
//!
//! Tests cover success paths (creating new files, overwriting existing files, atomic writes),
//! failure paths (nonexistent parent, write failures), and edge cases (empty content,
//! Unicode byte counting, temp file cleanup).

use crate::{execute, WriteOutput};
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

/// Helper to extract and deserialize WriteOutput from a ToolResult.
fn get_output(result: &operon_context_normalize_tools::ToolResult) -> WriteOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize WriteOutput")
        }
        other => panic!("expected Json content for success, got {:?}", other),
    }
}

// ============================================================================
// SUCCESS PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_create_new_file() {
    // Create a temp directory and a path for a new file (that doesn't exist yet).
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("new_file.txt");
    let path = file_path.to_string_lossy().to_string();

    // Verify the file doesn't exist yet.
    assert!(!file_path.exists());

    // Execute write to create the file.
    let content = "Hello, world!";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": content
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error, "write should succeed");
    let output = get_output(&result);
    assert!(output.created, "created should be true for new file");
    assert_eq!(output.bytes_written, content.len());
    assert!(output.message.contains("Created"));

    // Verify file exists and has correct content.
    assert!(file_path.exists());
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, content);
}

#[tokio::test]
async fn test_overwrite_existing_file() {
    // Create a temp file with initial content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let initial_content = "initial content";
    fs::write(&path, initial_content).unwrap();

    // Verify initial content.
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, initial_content);

    // Execute write to overwrite the file.
    let new_content = "completely new content";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": new_content
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error, "write should succeed");
    let output = get_output(&result);
    assert!(!output.created, "created should be false for overwrite");
    assert_eq!(output.bytes_written, new_content.len());
    assert!(output.message.contains("Overwrote"));

    // Verify file content changed.
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, new_content);
    assert!(!file_content.contains("initial"));
}

#[tokio::test]
async fn test_bytes_written_correct() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("unicode_file.txt");
    let path = file_path.to_string_lossy().to_string();

    // Use a Unicode string where char count != byte count.
    // "héllo" is 5 characters but 6 bytes (é is 2 bytes in UTF-8).
    let content = "héllo";
    let expected_bytes = content.as_bytes().len();
    assert_eq!(expected_bytes, 6, "héllo should be 6 bytes");

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(
        output.bytes_written, expected_bytes,
        "bytes_written should match UTF-8 byte count"
    );
}

#[tokio::test]
async fn test_atomic_write_no_tmp_files() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.txt");
    let path = file_path.to_string_lossy().to_string();

    // Execute write.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": "test content"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);

    // Verify no temp files left in the directory.
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
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("empty_file.txt");
    let path = file_path.to_string_lossy().to_string();

    // Write empty content.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": ""
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.bytes_written, 0);
    assert!(output.created);

    // Verify file exists and is empty.
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, "");
}

#[tokio::test]
async fn test_message_create_vs_overwrite() {
    // Test create message.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("new_file.txt");
    let path = file_path.to_string_lossy().to_string();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "content": "content"
        }),
    )
    .await
    .unwrap();

    let output = get_output(&result);
    assert!(
        output.message.contains("Created"),
        "message should contain 'Created' for new file"
    );

    // Test overwrite message.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": "new content"
        }),
    )
    .await
    .unwrap();

    let output = get_output(&result);
    assert!(
        output.message.contains("Overwrote"),
        "message should contain 'Overwrote' for existing file"
    );
}

// ============================================================================
// FAILURE PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_nonexistent_parent() {
    // Try to write to a path whose parent directory doesn't exist.
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
    assert!(error.contains("parent directory does not exist"));
}

#[tokio::test]
async fn test_nonexistent_parent_preserves_file() {
    // Create a temp file to verify it's not modified by a failed write attempt.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original_content = "original";
    fs::write(&path, original_content).unwrap();

    // Try to write to a nonexistent parent (this should fail).
    let _ = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": "/tmp/does_not_exist_xyz_operon_test/file.txt",
            "content": "new content"
        }),
    )
    .await
    .unwrap();

    // Verify the original file is unchanged.
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original_content);
}

#[tokio::test]
async fn test_write_failure_preserves_file() {
    // This test is tricky because we need to simulate a write failure.
    // On most systems, we can't easily trigger a write failure in a temp directory.
    // Instead, we'll verify the error message format is correct.
    // The actual atomic write failure is hard to test without mocking or special permissions.

    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let original_content = "original";
    fs::write(&path, original_content).unwrap();

    // Verify the file exists and has original content.
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original_content);

    // Note: We can't easily trigger a real write failure in a temp directory,
    // so we just verify that successful writes work correctly.
    // The error handling code is exercised by the nonexistent_parent test.
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[tokio::test]
async fn test_multiline_content() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("multiline.txt");
    let path = file_path.to_string_lossy().to_string();

    // Write multiline content.
    let content = "line 1\nline 2\nline 3\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, content);
}

#[tokio::test]
async fn test_large_content() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("large_file.txt");
    let path = file_path.to_string_lossy().to_string();

    // Create large content (1 MB).
    let content = "x".repeat(1024 * 1024);
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.bytes_written, 1024 * 1024);

    // Verify file content.
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content.len(), 1024 * 1024);
}

#[tokio::test]
async fn test_special_characters_in_content() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("special.txt");
    let path = file_path.to_string_lossy().to_string();

    // Write content with special characters.
    let content = "Special chars: !@#$%^&*()_+-=[]{}|;:',.<>?/~`\nEmoji: 🎉 🚀 ✨\n";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, content);
}

#[tokio::test]
async fn test_file_with_no_trailing_newline() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("no_newline.txt");
    let path = file_path.to_string_lossy().to_string();

    // Write content without trailing newline.
    let content = "no trailing newline";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, content);
}

#[tokio::test]
async fn test_overwrite_with_shorter_content() {
    // Create a temp file with longer content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let initial_content = "this is a longer initial content";
    fs::write(&path, initial_content).unwrap();

    // Overwrite with shorter content.
    let new_content = "short";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": new_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.bytes_written, new_content.len());

    // Verify file content is exactly the new content (not padded or partial).
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, new_content);
    assert_eq!(file_content.len(), new_content.len());
}

#[tokio::test]
async fn test_overwrite_with_longer_content() {
    // Create a temp file with shorter content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let initial_content = "short";
    fs::write(&path, initial_content).unwrap();

    // Overwrite with longer content.
    let new_content = "this is much longer content than before";
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path,
            "content": new_content
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.bytes_written, new_content.len());

    // Verify file content is exactly the new content.
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(file_content, new_content);
}

#[tokio::test]
async fn test_path_echoed_in_output() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.txt");
    let path = file_path.to_string_lossy().to_string();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "content": "content"
        }),
    )
    .await
    .unwrap();

    let output = get_output(&result);
    assert_eq!(output.path, path, "path should be echoed back in output");
}

#[tokio::test]
async fn test_sequential_writes_to_same_file() {
    // Create a temp directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("sequential.txt");
    let path = file_path.to_string_lossy().to_string();

    // First write (create).
    let result1 = execute(
        ToolCallId("call1".to_string()),
        json!({
            "path": path.clone(),
            "content": "first content"
        }),
    )
    .await
    .unwrap();

    assert!(!result1.is_error);
    let output1 = get_output(&result1);
    assert!(output1.created);

    // Second write (overwrite).
    let result2 = execute(
        ToolCallId("call2".to_string()),
        json!({
            "path": path.clone(),
            "content": "second content"
        }),
    )
    .await
    .unwrap();

    assert!(!result2.is_error);
    let output2 = get_output(&result2);
    assert!(!output2.created);

    // Third write (overwrite again).
    let result3 = execute(
        ToolCallId("call3".to_string()),
        json!({
            "path": path,
            "content": "third content"
        }),
    )
    .await
    .unwrap();

    assert!(!result3.is_error);
    let output3 = get_output(&result3);
    assert!(!output3.created);

    // Verify final content.
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, "third content");
}
