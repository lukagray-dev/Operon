/// Comprehensive tests for the read tool.
use crate::args::{parse_string_target, ReadTarget};
use crate::execute;
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper to create a temporary directory with test files.
fn setup_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();

    let simple_path = dir.path().join("simple.txt");
    fs::write(&simple_path, "line 1\nline 2\nline 3\nline 4\nline 5\n").unwrap();

    let no_newline_path = dir.path().join("no_newline.txt");
    fs::write(&no_newline_path, "line 1\nline 2\nline 3").unwrap();

    let binary_path = dir.path().join("binary.bin");
    fs::write(&binary_path, b"hello\x00world").unwrap();

    let large_path = dir.path().join("large.txt");
    let large_content = "x".repeat(1_048_577);
    fs::write(&large_path, large_content).unwrap();

    let empty_path = dir.path().join("empty.txt");
    fs::write(&empty_path, "").unwrap();

    dir
}

fn extract_text(result: operon_context_normalize_tools::ToolResult) -> String {
    match result.content {
        ToolContent::Text(t) => t,
        _ => panic!("Expected Text content"),
    }
}

#[test]
fn test_parse_string_target_variants() {
    assert_eq!(
        parse_string_target("src/main.rs:10-40"),
        ReadTarget {
            path: "src/main.rs".to_string(),
            start_line: Some(10),
            end_line: Some(40),
        }
    );
    assert_eq!(
        parse_string_target("src/main.rs:5-EOF"),
        ReadTarget {
            path: "src/main.rs".to_string(),
            start_line: Some(5),
            end_line: None,
        }
    );
    assert_eq!(
        parse_string_target("src/main.rs:15"),
        ReadTarget {
            path: "src/main.rs".to_string(),
            start_line: Some(15),
            end_line: Some(15),
        }
    );
    assert_eq!(
        parse_string_target(r"D:\Operon\src\main.rs:10-40"),
        ReadTarget {
            path: r"D:\Operon\src\main.rs".to_string(),
            start_line: Some(10),
            end_line: Some(40),
        }
    );
    assert_eq!(
        parse_string_target(r"D:\Operon\src\main.rs"),
        ReadTarget {
            path: r"D:\Operon\src\main.rs".to_string(),
            start_line: None,
            end_line: None,
        }
    );
}

#[tokio::test]
async fn test_read_single_file_full() {
    let dir = setup_test_dir();
    let path = dir.path().join("simple.txt");

    let args = json!({
        "path": path.to_str().unwrap()
    });

    let result = execute(ToolCallId("test_1".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result);
    assert!(text.contains(&format!("=== {} (5 lines) ===", path.to_str().unwrap())));
    assert!(text.contains("line 1\nline 2\nline 3\nline 4\nline 5"));
}

#[tokio::test]
async fn test_read_multiple_files_with_string_ranges() {
    let dir = setup_test_dir();
    let path1 = dir.path().join("simple.txt");
    let path2 = dir.path().join("no_newline.txt");

    let target1 = format!("{}:2-4", path1.to_str().unwrap());
    let target2 = format!("{}:2-EOF", path2.to_str().unwrap());

    let args = json!({
        "paths": [target1, target2]
    });

    let result = execute(ToolCallId("test_2".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result);
    assert!(text.contains(&format!("=== {} (lines 2-4 of 5) ===", path1.to_str().unwrap())));
    assert!(text.contains("line 2\nline 3\nline 4"));
    assert!(text.contains(&format!("=== {} (lines 2-3 of 3) ===", path2.to_str().unwrap())));
    assert!(text.contains("line 2\nline 3"));
}

#[tokio::test]
async fn test_relative_path_rejected() {
    let args = json!({
        "path": "relative/file.txt"
    });

    let result = execute(ToolCallId("test_4".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result);
    assert!(text.contains("=== relative/file.txt ==="));
    assert!(text.contains("Error: Path must be an absolute path"));
}

#[tokio::test]
async fn test_binary_file_detection() {
    let dir = setup_test_dir();
    let path = dir.path().join("binary.bin");

    let args = json!({
        "path": path.to_str().unwrap()
    });

    let result = execute(ToolCallId("test_5".to_string()), args)
        .await
        .unwrap();
    assert!(!result.is_error);

    let text = extract_text(result);
    assert!(text.contains("Error: Binary file detected"));
}
