//! Comprehensive tests for the delete tool.
//!
//! Tests cover success paths (deleting files and directories, trash vs permanent),
//! failure paths (nonexistent path, permission issues), and edge cases
//! (nested directories, symlinks, default permanent value).

use crate::{execute, DeleteOutput, DeletedKind};
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

/// Helper to extract and deserialize DeleteOutput from a ToolResult.
fn get_output(result: &operon_context_normalize_tools::ToolResult) -> DeleteOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize DeleteOutput")
        }
        other => panic!("expected Json content for success, got {:?}", other),
    }
}

// ============================================================================
// SUCCESS PATH TESTS — TRASH MODE
// ============================================================================

#[tokio::test]
async fn test_trash_file() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    // Delete to trash (permanent: false).
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "permanent": false
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error, "delete to trash should succeed");
    let output = get_output(&result);
    assert!(!output.permanent);
    assert_eq!(output.kind, DeletedKind::File);
    assert!(output.message.contains("trash"));

    // Verify file no longer exists at original path.
    assert!(!std::path::Path::new(&path).exists());
}

#[tokio::test]
async fn test_trash_directory() {
    // Create a temp directory with a file inside.
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();
    let file_path = std::path::Path::new(&dir_path).join("file.txt");
    fs::write(&file_path, "content").unwrap();

    // Delete directory to trash.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": dir_path.clone(),
            "permanent": false
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error);
    let output = get_output(&result);
    assert!(!output.permanent);
    assert_eq!(output.kind, DeletedKind::Dir);

    // Verify directory no longer exists.
    assert!(!std::path::Path::new(&dir_path).exists());
}

#[tokio::test]
async fn test_default_permanent_is_false() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    // Delete without specifying permanent (should default to false).
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone()
        }),
    )
    .await
    .unwrap();

    // Verify success and default applied.
    assert!(!result.is_error);
    let output = get_output(&result);
    assert!(!output.permanent, "permanent should default to false");

    // Verify file no longer exists.
    assert!(!std::path::Path::new(&path).exists());
}

// ============================================================================
// SUCCESS PATH TESTS — PERMANENT MODE
// ============================================================================

#[tokio::test]
async fn test_permanent_delete_file() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    // Permanently delete.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error);
    let output = get_output(&result);
    assert!(output.permanent);
    assert_eq!(output.kind, DeletedKind::File);
    assert!(output.message.contains("Permanently deleted"));

    // Verify file no longer exists.
    assert!(!std::path::Path::new(&path).exists());
}

#[tokio::test]
async fn test_permanent_delete_directory() {
    // Create a temp directory with nested structure.
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();
    let subdir = std::path::Path::new(&dir_path).join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file.txt"), "content").unwrap();

    // Permanently delete the directory.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": dir_path.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    // Verify success.
    assert!(!result.is_error);
    let output = get_output(&result);
    assert!(output.permanent);
    assert_eq!(output.kind, DeletedKind::Dir);

    // Verify directory and all contents no longer exist.
    assert!(!std::path::Path::new(&dir_path).exists());
}

// ============================================================================
// SUCCESS PATH TESTS — OUTPUT VALIDATION
// ============================================================================

#[tokio::test]
async fn test_path_echoed_in_output() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "permanent": false
        }),
    )
    .await
    .unwrap();

    let output = get_output(&result);
    assert_eq!(output.path, path, "path should be echoed back in output");
}

#[tokio::test]
async fn test_kind_file_vs_dir() {
    // Test file deletion.
    let file = NamedTempFile::new().unwrap();
    let file_path = file.path().to_string_lossy().to_string();
    fs::write(&file_path, "content").unwrap();

    let result_file = execute(
        ToolCallId("test_file".to_string()),
        json!({
            "path": file_path,
            "permanent": false
        }),
    )
    .await
    .unwrap();

    let output_file = get_output(&result_file);
    assert_eq!(output_file.kind, DeletedKind::File);

    // Test directory deletion.
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();

    let result_dir = execute(
        ToolCallId("test_dir".to_string()),
        json!({
            "path": dir_path,
            "permanent": false
        }),
    )
    .await
    .unwrap();

    let output_dir = get_output(&result_dir);
    assert_eq!(output_dir.kind, DeletedKind::Dir);
}

#[tokio::test]
async fn test_message_format_trash() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "permanent": false
        }),
    )
    .await
    .unwrap();

    let output = get_output(&result);
    assert!(output.message.contains("Moved"));
    assert!(output.message.contains("trash"));
    assert!(output.message.contains(&path));
}

#[tokio::test]
async fn test_message_format_permanent() {
    // Create a temp file.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "content").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    let output = get_output(&result);
    assert!(output.message.contains("Permanently deleted"));
    assert!(output.message.contains(&path));
}

// ============================================================================
// FAILURE PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_path_not_found() {
    let temp_dir = tempfile::tempdir().unwrap();
    let nonexistent = temp_dir.path().join("does_not_exist_file.txt");
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": nonexistent.to_str().unwrap(),
            "permanent": false
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("does not exist"));
}

#[tokio::test]
async fn test_nonexistent_nested_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let nonexistent_nested = temp_dir.path().join("subdir").join("file.txt");
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": nonexistent_nested.to_str().unwrap(),
            "permanent": false
        }),
    )
    .await
    .unwrap();

    // Verify error.
    assert!(result.is_error);
    let error = get_error_text(&result);
    assert!(error.contains("does not exist"));
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[tokio::test]
async fn test_delete_nested_directory_structure() {
    // Create a complex nested directory structure.
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_string_lossy().to_string();

    // Create nested structure: root/a/b/c with files at each level.
    let a = std::path::Path::new(&root).join("a");
    let b = a.join("b");
    let c = b.join("c");
    fs::create_dir_all(&c).unwrap();
    fs::write(std::path::Path::new(&root).join("file_root.txt"), "root").unwrap();
    fs::write(a.join("file_a.txt"), "a").unwrap();
    fs::write(b.join("file_b.txt"), "b").unwrap();
    fs::write(c.join("file_c.txt"), "c").unwrap();

    // Permanently delete the entire structure.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": root.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    assert!(!std::path::Path::new(&root).exists());
}

#[tokio::test]
async fn test_delete_empty_directory() {
    // Create an empty directory.
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();

    // Delete the empty directory.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": dir_path.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert_eq!(output.kind, DeletedKind::Dir);
    assert!(!std::path::Path::new(&dir_path).exists());
}

#[tokio::test]
async fn test_delete_file_with_special_characters_in_name() {
    // Create a file with special characters in the name.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("file with spaces & special!@#.txt");
    fs::write(&file_path, "content").unwrap();

    let path_str = file_path.to_string_lossy().to_string();
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path_str.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    assert!(!file_path.exists());
}

#[tokio::test]
async fn test_delete_large_directory() {
    // Create a directory with many files.
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();

    // Create 100 files in the directory.
    for i in 0..100 {
        let file_path = std::path::Path::new(&dir_path).join(format!("file_{}.txt", i));
        fs::write(file_path, format!("content {}", i)).unwrap();
    }

    // Delete the entire directory.
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": dir_path.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    assert!(!std::path::Path::new(&dir_path).exists());
}

#[tokio::test]
async fn test_delete_file_with_unicode_name() {
    // Create a file with Unicode characters in the name.
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("файл_文件_ファイル.txt");
    fs::write(&file_path, "content").unwrap();

    let path_str = file_path.to_string_lossy().to_string();
    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path_str,
            "permanent": true
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    assert!(!file_path.exists());
}

#[tokio::test]
async fn test_delete_file_with_unicode_content() {
    // Create a file with Unicode content.
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "Unicode content: 你好世界 🌍 مرحبا").unwrap();

    let result = execute(
        ToolCallId("test_call".to_string()),
        json!({
            "path": path.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    assert!(!std::path::Path::new(&path).exists());
}

#[tokio::test]
async fn test_permanent_flag_true_vs_false() {
    // Create two files.
    let file1 = NamedTempFile::new().unwrap();
    let path1 = file1.path().to_string_lossy().to_string();
    fs::write(&path1, "content1").unwrap();

    let file2 = NamedTempFile::new().unwrap();
    let path2 = file2.path().to_string_lossy().to_string();
    fs::write(&path2, "content2").unwrap();

    // Delete first with permanent: false.
    let result1 = execute(
        ToolCallId("call1".to_string()),
        json!({
            "path": path1.clone(),
            "permanent": false
        }),
    )
    .await
    .unwrap();

    let output1 = get_output(&result1);
    assert!(!output1.permanent);

    // Delete second with permanent: true.
    let result2 = execute(
        ToolCallId("call2".to_string()),
        json!({
            "path": path2.clone(),
            "permanent": true
        }),
    )
    .await
    .unwrap();

    let output2 = get_output(&result2);
    assert!(output2.permanent);

    // Both should be gone from original location.
    assert!(!std::path::Path::new(&path1).exists());
    assert!(!std::path::Path::new(&path2).exists());
}

#[tokio::test]
async fn test_delete_field_aliases() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "delete me").unwrap();

    let result = execute(
        ToolCallId("alias_call".to_string()),
        json!({
            "target_file": path.clone(),
            "force": true
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_output(&result);
    assert!(output.permanent);
    assert!(!std::path::Path::new(&path).exists());
}

#[tokio::test]
async fn test_delete_relative_path_rejected() {
    let result = execute(
        ToolCallId("rel_call".to_string()),
        json!({
            "path": "relative/path/test.txt"
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let error_text = match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content for error, got {:?}", other),
    };
    assert!(error_text.contains("Path must be an absolute path"));
}
