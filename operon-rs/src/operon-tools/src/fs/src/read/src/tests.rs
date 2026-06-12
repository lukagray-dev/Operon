/// Comprehensive tests for the read tool.
///
/// These tests cover all major functionality using the plain-text semicolon-delimited format:
/// - Full-file reads (raw output for single-file, path-header prefixed for multi-file)
/// - Line-range reads (clamped ranges, start-only, end-only, bounds checks)
/// - Binary file detection (detecting null bytes and returning inline error)
/// - Size limit enforcement (blocking full-file reads over 1 MB, allowing range reads)
/// - Error handling (non-existent files, range bounds exceeded, malformed arguments)
/// - Concurrent multi-file reads (up to 16 concurrently bounded by a semaphore)
/// - Semicolon-delimited mixed path parsing
/// - Ledger updating (read_paths returned correctly in ToolResult)
use crate::execute;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper to create a temporary directory with various files for testing.
fn setup_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();

    // 1. Create a simple text file with exactly 5 lines (newlines at the end of every line)
    let simple_path = dir.path().join("simple.txt");
    fs::write(&simple_path, "line 1\nline 2\nline 3\nline 4\nline 5\n").unwrap();

    // 2. Create a text file without a trailing newline at the end of the last line
    let no_newline_path = dir.path().join("no_newline.txt");
    fs::write(&no_newline_path, "line 1\nline 2\nline 3").unwrap();

    // 3. Create a binary file (containing a null byte) to test binary safety checking
    let binary_path = dir.path().join("binary.bin");
    fs::write(&binary_path, b"hello\x00world").unwrap();

    // 4. Create a large text file exceeding the 1 MB safety limit (1 MB + 2 bytes)
    let large_path = dir.path().join("large.txt");
    let large_content = "x\n".repeat(524289);
    fs::write(&large_path, large_content).unwrap();

    // 5. Create an empty file to test boundary edge cases
    let empty_path = dir.path().join("empty.txt");
    fs::write(&empty_path, "").unwrap();

    dir
}

/// Helper to extract the plain text content from a ToolResult.
/// Panics if the content is not of type ToolContent::Text.
fn extract_text(result: ToolResult) -> String {
    match result.content {
        ToolContent::Text(t) => t,
        _ => panic!("Expected plain text ToolContent::Text output format"),
    }
}

#[tokio::test]
async fn test_read_single_file_full() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");

    // Single paths are passed directly as a string under the "paths" attribute
    let args = json!({
        "paths": path.to_str().unwrap()
    });

    let result = execute(ToolCallId("test_1".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    // Single-file full reads should return the raw file content directly (no headers)
    let text = extract_text(result.clone());
    assert_eq!(text, "line 1\nline 2\nline 3\nline 4\nline 5\n");

    // The read ledger paths must be populated with the path of the successfully read file
    let paths = result.read_paths.unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], path.to_str().unwrap());
}

#[tokio::test]
async fn test_read_multiple_files() {
    let dir = setup_test_dir();
    let path1 = dir.path().join("simple.txt");
    let path2 = dir.path().join("no_newline.txt");

    // Semicolon-delimited list of files
    let args = json!({
        "paths": format!("{}; {}", path1.to_str().unwrap(), path2.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_2".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    // Multi-file full reads must include path-header lines for each file
    let text = extract_text(result.clone());
    let expected = format!(
        "{}\nline 1\nline 2\nline 3\nline 4\nline 5\n\n\n{}\nline 1\nline 2\nline 3",
        path1.to_str().unwrap(),
        path2.to_str().unwrap()
    );
    assert_eq!(text, expected);

    // Both files were successfully read, so both paths must be in the ledger
    let paths = result.read_paths.unwrap();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&path1.to_str().unwrap().to_string()));
    assert!(paths.contains(&path2.to_str().unwrap().to_string()));
}

#[tokio::test]
async fn test_read_with_line_range() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");

    // Specify a line range: paths are appended with :START-END
    let args = json!({
        "paths": format!("{}:2-4", path.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_3".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    // Range reads should include range information in the path header line
    let text = extract_text(result.clone());
    let expected_header = format!("{} lines 2-4 of 5\n", path.to_str().unwrap());
    assert!(text.starts_with(&expected_header));
    assert!(text.contains("line 2\nline 3\nline 4\n"));

    // File was read successfully, so the ledger should record the path
    let paths = result.read_paths.unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], path.to_str().unwrap());
}

#[tokio::test]
async fn test_read_with_start_line_only() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");

    // Start line only (read from start_line to EOF)
    let args = json!({
        "paths": format!("{}:3-", path.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_4".to_string()), args)
        .await
        .unwrap();
    let text = extract_text(result);
    let expected_header = format!("{} lines 3-5 of 5\n", path.to_str().unwrap());
    assert!(text.starts_with(&expected_header));
    assert!(text.contains("line 3\nline 4\nline 5\n"));
}

#[tokio::test]
async fn test_read_with_end_line_only() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");

    // End line only (read from line 1 to end_line)
    let args = json!({
        "paths": format!("{}:-3", path.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_5".to_string()), args)
        .await
        .unwrap();
    let text = extract_text(result);
    let expected_header = format!("{} lines 1-3 of 5\n", path.to_str().unwrap());
    assert!(text.starts_with(&expected_header));
    assert!(text.contains("line 1\nline 2\nline 3\n"));
}

#[tokio::test]
async fn test_read_line_range_exceeds_file() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");

    // End line beyond file length should be clamped silently to EOF (last line)
    let args = json!({
        "paths": format!("{}:3-100", path.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_6".to_string()), args)
        .await
        .unwrap();
    let text = extract_text(result);
    let expected_header = format!("{} lines 3-5 of 5\n", path.to_str().unwrap());
    assert!(text.starts_with(&expected_header));
    assert!(text.contains("line 3\nline 4\nline 5\n"));
}

#[tokio::test]
async fn test_read_start_line_exceeds_file() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");

    // Start line exceeding file length should trigger a per-file inline error
    let args = json!({
        "paths": format!("{}:100-", path.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_7".to_string()), args)
        .await
        .unwrap();
    // The tool call does not fail overall, is_error remains false
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    let expected_error = format!(
        "{}\nERROR: start_line 100 exceeds file length (5 lines).",
        path.to_str().unwrap()
    );
    assert_eq!(text, expected_error);

    // This file read failed, so it should NOT be in the read paths list
    let paths = result.read_paths.unwrap();
    assert!(paths.is_empty());
}

#[tokio::test]
async fn test_read_binary_file() {
    let dir = setup_test_dir();
    let path = dir.path().join("binary.bin");

    let args = json!({
        "paths": path.to_str().unwrap()
    });

    let result = execute(ToolCallId("test_8".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    assert!(text.contains("ERROR: Binary file detected"));

    // Failed read due to binary validation must not update read paths
    let paths = result.read_paths.unwrap();
    assert!(paths.is_empty());
}

#[tokio::test]
async fn test_read_large_file_without_range() {
    let dir = setup_test_dir();
    let path = dir.path().join("large.txt");

    let args = json!({
        "paths": path.to_str().unwrap()
    });

    let result = execute(ToolCallId("test_9".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    assert!(text.contains("ERROR: File exceeds 1 MB limit"));

    // Failed read due to size check must not update read paths
    let paths = result.read_paths.unwrap();
    assert!(paths.is_empty());
}

#[tokio::test]
async fn test_read_large_file_with_range() {
    let dir = setup_test_dir();
    let path = dir.path().join("large.txt");

    // Range reads bypass the 1 MB size check
    let args = json!({
        "paths": format!("{}:1-10", path.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_10".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    assert!(text.contains("lines 1-10 of"));
    assert!(!text.contains("ERROR"));

    // Since it bypassed the check and read successfully, path is recorded
    let paths = result.read_paths.unwrap();
    assert_eq!(paths.len(), 1);
}

#[tokio::test]
async fn test_read_nonexistent_file() {
    let args = json!({
        "paths": "/nonexistent/path/to/file.txt"
    });

    let result = execute(ToolCallId("test_11".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    assert!(text.contains("ERROR: Failed to access file") || text.contains("ERROR: Failed to read file"));

    let paths = result.read_paths.unwrap();
    assert!(paths.is_empty());
}

#[tokio::test]
async fn test_read_empty_file() {
    let dir = setup_test_dir();
    let path = dir.path().join("empty.txt");

    let args = json!({
        "paths": path.to_str().unwrap()
    });

    let result = execute(ToolCallId("test_12".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    assert_eq!(text, ""); // Empty file has empty output

    let paths = result.read_paths.unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], path.to_str().unwrap());
}

#[tokio::test]
async fn test_read_mixed_string_targets() {
    let dir = setup_test_dir();
    let path1 = dir.path().join("simple.txt");
    let path2 = dir.path().join("no_newline.txt");

    // Mix full-file read path and range read path in the same paths string
    let args = json!({
        "paths": format!("{}; {}:2-3", path1.to_str().unwrap(), path2.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_13".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    assert!(text.contains("simple.txt"));
    assert!(text.contains("no_newline.txt lines 2-3 of 3"));
    assert!(text.contains("line 2\nline 3"));

    let paths = result.read_paths.unwrap();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&path1.to_str().unwrap().to_string()));
    assert!(paths.contains(&path2.to_str().unwrap().to_string()));
}

#[tokio::test]
async fn test_read_partial_failure() {
    let dir = setup_test_dir();
    let path1 = dir.path().join("simple.txt");
    let path2 = "/nonexistent/file.txt";
    let path3 = dir.path().join("no_newline.txt");

    let args = json!({
        "paths": format!("{}; {}; {}", path1.to_str().unwrap(), path2, path3.to_str().unwrap())
    });

    let result = execute(ToolCallId("test_14".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    assert!(text.contains("simple.txt"));
    assert!(text.contains("/nonexistent/file.txt\nERROR:"));
    assert!(text.contains("no_newline.txt"));

    // Successful ones are recorded in the ledger, failed one is not
    let paths = result.read_paths.unwrap();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&path1.to_str().unwrap().to_string()));
    assert!(paths.contains(&path3.to_str().unwrap().to_string()));
}

#[tokio::test]
async fn test_invalid_args_format() {
    // Missing required paths key should trigger argument parsing failure
    let args = json!({
        "invalid": "field"
    });

    let result = execute(ToolCallId("test_15".to_string()), args).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_reads() {
    let dir = setup_test_dir();

    // Create 10 files to verify concurrent processing logic
    let mut paths = vec![];
    for i in 0..10 {
        let path = dir.path().join(format!("file_{}.txt", i));
        fs::write(&path, format!("content {}", i)).unwrap();
        paths.push(path.to_str().unwrap().to_string());
    }

    let args = json!({
        "paths": paths.join("; ")
    });

    let result = execute(ToolCallId("test_16".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result.clone());
    for i in 0..10 {
        assert!(text.contains(&paths[i]));
        assert!(text.contains(&format!("content {}", i)));
    }

    let read_paths = result.read_paths.unwrap();
    assert_eq!(read_paths.len(), 10);
}
