/// Comprehensive tests for the read tool.
///
/// These tests cover all major functionality:
/// - Full-file reads
/// - Line-range reads
/// - Binary file detection
/// - Size limit enforcement
/// - Error handling (missing files, invalid ranges, etc.)
/// - Concurrent multi-file reads
/// - Mixed input formats (plain strings and objects)

use crate::{execute, ReadOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper to create a temporary directory with test files.
fn setup_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    
    // Create a simple text file
    let simple_path = dir.path().join("simple.txt");
    fs::write(&simple_path, "line 1\nline 2\nline 3\nline 4\nline 5\n").unwrap();
    
    // Create a file without trailing newline
    let no_newline_path = dir.path().join("no_newline.txt");
    fs::write(&no_newline_path, "line 1\nline 2\nline 3").unwrap();
    
    // Create a binary file (with null bytes)
    let binary_path = dir.path().join("binary.bin");
    fs::write(&binary_path, b"hello\x00world").unwrap();
    
    // Create a large file (over 1 MB)
    let large_path = dir.path().join("large.txt");
    let large_content = "x".repeat(1_048_577); // 1 MB + 1 byte
    fs::write(&large_path, large_content).unwrap();
    
    // Create an empty file
    let empty_path = dir.path().join("empty.txt");
    fs::write(&empty_path, "").unwrap();
    
    dir
}

/// Helper to extract ReadOutput from a ToolResult.
fn extract_output(result: operon_context_normalize_tools::ToolResult) -> ReadOutput {
    match result.content {
        ToolContent::Json(v) => serde_json::from_value(v).unwrap(),
        _ => panic!("Expected JSON content"),
    }
}

#[tokio::test]
async fn test_read_single_file_full() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");
    
    let args = json!({
        "paths": [path.to_str().unwrap()]
    });
    
    let result = execute(ToolCallId("test_1".to_string()), args).await.unwrap();
    assert!(!result.is_error);
    
    let output = extract_output(result);
    assert_eq!(output.files.len(), 1);
    
    let file_result = &output.files[0];
    assert!(file_result.success);
    assert_eq!(file_result.content.as_ref().unwrap(), "line 1\nline 2\nline 3\nline 4\nline 5\n");
    assert_eq!(file_result.total_lines, Some(5));
    assert!(file_result.lines_returned.is_none());
    assert!(file_result.error.is_none());
}

#[tokio::test]
async fn test_read_multiple_files() {
    let dir = setup_test_dir();
    let path1 = dir.path().join("simple.txt");
    let path2 = dir.path().join("no_newline.txt");
    
    let args = json!({
        "paths": [path1.to_str().unwrap(), path2.to_str().unwrap()]
    });
    
    let result = execute(ToolCallId("test_2".to_string()), args).await.unwrap();
    assert!(!result.is_error);
    
    let output = extract_output(result);
    assert_eq!(output.files.len(), 2);
    
    // First file
    assert!(output.files[0].success);
    assert_eq!(output.files[0].total_lines, Some(5));
    
    // Second file
    assert!(output.files[1].success);
    assert_eq!(output.files[1].total_lines, Some(3));
    assert_eq!(output.files[1].content.as_ref().unwrap(), "line 1\nline 2\nline 3");
}

#[tokio::test]
async fn test_read_with_line_range() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");
    
    let args = json!({
        "paths": [{
            "path": path.to_str().unwrap(),
            "start_line": 2,
            "end_line": 4
        }]
    });
    
    let result = execute(ToolCallId("test_3".to_string()), args).await.unwrap();
    assert!(!result.is_error);
    
    let output = extract_output(result);
    assert_eq!(output.files.len(), 1);
    
    let file_result = &output.files[0];
    assert!(file_result.success);
    assert_eq!(file_result.content.as_ref().unwrap(), "line 2\nline 3\nline 4\n");
    assert_eq!(file_result.total_lines, Some(5));
    
    let range = file_result.lines_returned.as_ref().unwrap();
    assert_eq!(range.start, 2);
    assert_eq!(range.end, 4);
}

#[tokio::test]
async fn test_read_with_start_line_only() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");
    
    let args = json!({
        "paths": [{
            "path": path.to_str().unwrap(),
            "start_line": 3
        }]
    });
    
    let result = execute(ToolCallId("test_4".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    assert!(file_result.success);
    assert_eq!(file_result.content.as_ref().unwrap(), "line 3\nline 4\nline 5\n");
    
    let range = file_result.lines_returned.as_ref().unwrap();
    assert_eq!(range.start, 3);
    assert_eq!(range.end, 5);
}

#[tokio::test]
async fn test_read_with_end_line_only() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");
    
    let args = json!({
        "paths": [{
            "path": path.to_str().unwrap(),
            "end_line": 3
        }]
    });
    
    let result = execute(ToolCallId("test_5".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    assert!(file_result.success);
    assert_eq!(file_result.content.as_ref().unwrap(), "line 1\nline 2\nline 3\n");
    
    let range = file_result.lines_returned.as_ref().unwrap();
    assert_eq!(range.start, 1);
    assert_eq!(range.end, 3);
}

#[tokio::test]
async fn test_read_line_range_exceeds_file() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");
    
    // Request lines 3-100, should clamp to 3-5
    let args = json!({
        "paths": [{
            "path": path.to_str().unwrap(),
            "start_line": 3,
            "end_line": 100
        }]
    });
    
    let result = execute(ToolCallId("test_6".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    assert!(file_result.success);
    assert_eq!(file_result.content.as_ref().unwrap(), "line 3\nline 4\nline 5\n");
    
    let range = file_result.lines_returned.as_ref().unwrap();
    assert_eq!(range.start, 3);
    assert_eq!(range.end, 5); // Clamped to actual file length
}

#[tokio::test]
async fn test_read_start_line_exceeds_file() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");
    
    // Request starting from line 100 (file only has 5 lines)
    let args = json!({
        "paths": [{
            "path": path.to_str().unwrap(),
            "start_line": 100
        }]
    });
    
    let result = execute(ToolCallId("test_7".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    assert!(!file_result.success);
    assert!(file_result.error.as_ref().unwrap().contains("exceeds file length"));
    assert_eq!(file_result.total_lines, Some(5));
}

#[tokio::test]
async fn test_read_binary_file() {
    let dir = setup_test_dir();
    let path = dir.path().join("binary.bin");
    
    let args = json!({
        "paths": [path.to_str().unwrap()]
    });
    
    let result = execute(ToolCallId("test_8".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    assert!(!file_result.success);
    assert!(file_result.error.as_ref().unwrap().contains("Binary file detected"));
}

#[tokio::test]
async fn test_read_large_file_without_range() {
    let dir = setup_test_dir();
    let path = dir.path().join("large.txt");
    
    let args = json!({
        "paths": [path.to_str().unwrap()]
    });
    
    let result = execute(ToolCallId("test_9".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    assert!(!file_result.success);
    assert!(file_result.error.as_ref().unwrap().contains("exceeds 1 MB limit"));
    assert!(file_result.error.as_ref().unwrap().contains("Use start_line/end_line"));
}

#[tokio::test]
async fn test_read_large_file_with_range() {
    let dir = setup_test_dir();
    let path = dir.path().join("large.txt");
    
    // Reading with a range should bypass the size check
    let args = json!({
        "paths": [{
            "path": path.to_str().unwrap(),
            "start_line": 1,
            "end_line": 1
        }]
    });
    
    let result = execute(ToolCallId("test_10".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    // Should succeed because we're using a line range (even though the file is large)
    assert!(file_result.success);
}

#[tokio::test]
async fn test_read_nonexistent_file() {
    let args = json!({
        "paths": ["/nonexistent/path/to/file.txt"]
    });
    
    let result = execute(ToolCallId("test_11".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    assert!(!file_result.success);
    assert!(file_result.error.is_some());
}

#[tokio::test]
async fn test_read_empty_file() {
    let dir = setup_test_dir();
    let path = dir.path().join("empty.txt");
    
    let args = json!({
        "paths": [path.to_str().unwrap()]
    });
    
    let result = execute(ToolCallId("test_12".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    let file_result = &output.files[0];
    assert!(file_result.success);
    assert_eq!(file_result.content.as_ref().unwrap(), "");
    assert_eq!(file_result.total_lines, Some(0));
}

#[tokio::test]
async fn test_read_mixed_input_formats() {
    let dir = setup_test_dir();
    let path1 = dir.path().join("simple.txt");
    let path2 = dir.path().join("no_newline.txt");
    
    // Mix plain strings and objects
    let args = json!({
        "paths": [
            path1.to_str().unwrap(),
            {
                "path": path2.to_str().unwrap(),
                "start_line": 2,
                "end_line": 3
            }
        ]
    });
    
    let result = execute(ToolCallId("test_13".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    assert_eq!(output.files.len(), 2);
    
    // First file (plain string, full read)
    assert!(output.files[0].success);
    assert!(output.files[0].lines_returned.is_none());
    
    // Second file (object with range)
    assert!(output.files[1].success);
    assert!(output.files[1].lines_returned.is_some());
    assert_eq!(output.files[1].content.as_ref().unwrap(), "line 2\nline 3");
}

#[tokio::test]
async fn test_read_partial_failure() {
    let dir = setup_test_dir();
    let path1 = dir.path().join("simple.txt");
    let path2 = "/nonexistent/file.txt";
    let path3 = dir.path().join("no_newline.txt");
    
    let args = json!({
        "paths": [
            path1.to_str().unwrap(),
            path2,
            path3.to_str().unwrap()
        ]
    });
    
    let result = execute(ToolCallId("test_14".to_string()), args).await.unwrap();
    // Tool call itself should succeed (is_error: false)
    assert!(!result.is_error);
    
    let output = extract_output(result);
    assert_eq!(output.files.len(), 3);
    
    // First file: success
    assert!(output.files[0].success);
    
    // Second file: failure
    assert!(!output.files[1].success);
    assert!(output.files[1].error.is_some());
    
    // Third file: success
    assert!(output.files[2].success);
}

#[tokio::test]
async fn test_invalid_args_format() {
    // Missing "paths" field
    let args = json!({
        "invalid": "field"
    });
    
    let result = execute(ToolCallId("test_15".to_string()), args).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_reads() {
    let dir = setup_test_dir();
    
    // Create multiple files to read concurrently
    let mut paths = vec![];
    for i in 0..10 {
        let path = dir.path().join(format!("file_{}.txt", i));
        fs::write(&path, format!("content {}", i)).unwrap();
        paths.push(path.to_str().unwrap().to_string());
    }
    
    let args = json!({
        "paths": paths
    });
    
    let result = execute(ToolCallId("test_16".to_string()), args).await.unwrap();
    let output = extract_output(result);
    
    assert_eq!(output.files.len(), 10);
    
    // All reads should succeed
    for (i, file_result) in output.files.iter().enumerate() {
        assert!(file_result.success, "File {} failed", i);
        assert_eq!(file_result.content.as_ref().unwrap(), &format!("content {}", i));
    }
}
