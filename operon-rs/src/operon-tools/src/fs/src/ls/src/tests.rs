//! Tests for the ls tool.
//!
//! Comprehensive test suite covering basic listing, exclusion patterns, error cases,
//! hidden files, truncation, and metadata collection.

use crate::{execute, LsOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper function to create a test directory structure and execute ls.
async fn list_temp_dir(
    setup: impl Fn(&TempDir) -> std::io::Result<()>,
    path_suffix: &str,
    ignore: Option<Vec<String>>,
) -> LsOutput {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    setup(&temp_dir).expect("failed to set up test directory");

    let path = temp_dir
        .path()
        .join(path_suffix)
        .to_string_lossy()
        .to_string();

    let args = if let Some(ignore_patterns) = ignore {
        json!({
            "path": path,
            "ignore": ignore_patterns
        })
    } else {
        json!({
            "path": path
        })
    };

    let result = execute(ToolCallId("test_call".to_string()), args)
        .await
        .expect("execute failed");

    assert_eq!(result.name, "ls");
    assert!(!result.is_error);

    // Extract the JSON content and deserialize to LsOutput.
    match result.content {
        ToolContent::Json(value) => {
            serde_json::from_value(value).expect("failed to deserialize LsOutput")
        }
        _ => panic!("expected JSON content"),
    }
}

#[tokio::test]
async fn test_basic_listing() {
    // Create a temp directory with 2 files and 1 subdirectory.
    let output = list_temp_dir(
        |temp_dir| {
            fs::write(temp_dir.path().join("file1.txt"), "content1")?;
            fs::write(temp_dir.path().join("file2.txt"), "content2")?;
            fs::create_dir(temp_dir.path().join("subdir"))?;
            Ok(())
        },
        "",
        None,
    )
    .await;

    // Verify the output.
    assert_eq!(output.entry_count, 3);
    assert!(!output.truncated);
    assert!(output.error.is_none());

    // Verify entries are sorted: dir first, then files.
    assert_eq!(output.entries.len(), 3);
    assert_eq!(output.entries[0].name, "subdir");
    assert_eq!(output.entries[0].kind, crate::EntryKind::Dir);
    assert_eq!(output.entries[1].name, "file1.txt");
    assert_eq!(output.entries[1].kind, crate::EntryKind::File);
    assert_eq!(output.entries[2].name, "file2.txt");
    assert_eq!(output.entries[2].kind, crate::EntryKind::File);

    // Verify file sizes are populated.
    assert_eq!(output.entries[1].size_bytes, Some(8));
    assert_eq!(output.entries[2].size_bytes, Some(8));

    // Verify directory has no size.
    assert_eq!(output.entries[0].size_bytes, None);
}

#[tokio::test]
async fn test_ignore_patterns() {
    // Create a directory with files and subdirectories that will be excluded.
    let output = list_temp_dir(
        |temp_dir| {
            fs::write(temp_dir.path().join("Cargo.lock"), "")?;
            fs::create_dir(temp_dir.path().join("node_modules"))?;
            fs::create_dir(temp_dir.path().join("src"))?;
            fs::write(temp_dir.path().join("main.rs"), "")?;
            Ok(())
        },
        "",
        Some(vec!["*.lock".to_string(), "node_modules".to_string()]),
    )
    .await;

    // Verify that excluded entries don't appear.
    assert_eq!(output.entry_count, 2);
    assert!(!output.truncated);
    assert!(output.error.is_none());

    let names: Vec<_> = output.entries.iter().map(|e| &e.name).collect();
    assert!(!names.contains(&&"Cargo.lock".to_string()));
    assert!(!names.contains(&&"node_modules".to_string()));
    assert!(names.contains(&&"src".to_string()));
    assert!(names.contains(&&"main.rs".to_string()));
}

#[tokio::test]
async fn test_file_path_error() {
    // Create a temp directory with a file.
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, "content").expect("failed to write file");

    let args = json!({
        "path": file_path.to_string_lossy().to_string()
    });

    let result = execute(ToolCallId("test_call".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);

    // Extract the JSON content and deserialize to LsOutput.
    let output: LsOutput = match result.content {
        ToolContent::Json(value) => serde_json::from_value(value).expect("failed to deserialize"),
        _ => panic!("expected JSON content"),
    };

    // Verify error is set and entries are empty.
    assert!(output.error.is_some());
    assert!(output.error.unwrap().contains("file, not a directory"));
    assert_eq!(output.entry_count, 0);
    assert!(output.entries.is_empty());
}

#[tokio::test]
async fn test_nonexistent_path() {
    let args = json!({
        "path": "/nonexistent/path/that/does/not/exist"
    });

    let result = execute(ToolCallId("test_call".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);

    // Extract the JSON content and deserialize to LsOutput.
    let output: LsOutput = match result.content {
        ToolContent::Json(value) => serde_json::from_value(value).expect("failed to deserialize"),
        _ => panic!("expected JSON content"),
    };

    // Verify error is set.
    assert!(output.error.is_some());
    assert!(output.error.unwrap().contains("not found"));
    assert_eq!(output.entry_count, 0);
    assert!(output.entries.is_empty());
}

#[tokio::test]
async fn test_hidden_files_included() {
    // Create a directory with hidden and normal files.
    let output = list_temp_dir(
        |temp_dir| {
            fs::write(temp_dir.path().join(".env"), "secret")?;
            fs::write(temp_dir.path().join("normal.txt"), "content")?;
            Ok(())
        },
        "",
        None,
    )
    .await;

    // Verify both hidden and normal files are included.
    assert_eq!(output.entry_count, 2);
    assert!(!output.truncated);
    assert!(output.error.is_none());

    let names: Vec<_> = output.entries.iter().map(|e| &e.name).collect();
    assert!(names.contains(&&".env".to_string()));
    assert!(names.contains(&&"normal.txt".to_string()));
}

#[tokio::test]
async fn test_truncation() {
    // Create a directory with more than MAX_ENTRIES (1000) entries.
    let output = list_temp_dir(
        |temp_dir| {
            for i in 0..1100 {
                fs::write(temp_dir.path().join(format!("file_{:04}.txt", i)), "")?;
            }
            Ok(())
        },
        "",
        None,
    )
    .await;

    // Verify truncation is set and entry count is capped.
    assert!(output.truncated);
    assert_eq!(output.entries.len(), 1000);
    assert_eq!(output.entry_count, 1000);
    assert!(output.error.is_none());
}

#[tokio::test]
async fn test_metadata_collection() {
    // Create a file and verify metadata is collected.
    let output = list_temp_dir(
        |temp_dir| {
            fs::write(temp_dir.path().join("test.txt"), "hello world")?;
            Ok(())
        },
        "",
        None,
    )
    .await;

    assert_eq!(output.entry_count, 1);
    let entry = &output.entries[0];

    // Verify file metadata.
    assert_eq!(entry.name, "test.txt");
    assert_eq!(entry.kind, crate::EntryKind::File);
    assert_eq!(entry.size_bytes, Some(11)); // "hello world" is 11 bytes
    assert!(entry.modified_unix.is_some());
    assert!(entry.modified_unix.unwrap() > 0);
}

#[tokio::test]
async fn test_invalid_ignore_pattern() {
    // Create a simple directory.
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    fs::write(temp_dir.path().join("test.txt"), "").expect("failed to write file");

    let args = json!({
        "path": temp_dir.path().to_string_lossy().to_string(),
        "ignore": ["[invalid(pattern"]
    });

    let result = execute(ToolCallId("test_call".to_string()), args)
        .await
        .expect("execute failed");

    assert!(!result.is_error);

    // Extract the JSON content and deserialize to LsOutput.
    let output: LsOutput = match result.content {
        ToolContent::Json(value) => serde_json::from_value(value).expect("failed to deserialize"),
        _ => panic!("expected JSON content"),
    };

    // Verify error is set for invalid pattern.
    assert!(output.error.is_some());
    assert!(output.error.unwrap().contains("invalid ignore pattern"));
    assert_eq!(output.entry_count, 0);
    assert!(output.entries.is_empty());
}

#[tokio::test]
async fn test_sorting_case_insensitive() {
    // Create files with mixed case names.
    let output = list_temp_dir(
        |temp_dir| {
            fs::create_dir(temp_dir.path().join("Zebra"))?;
            fs::create_dir(temp_dir.path().join("apple"))?;
            fs::write(temp_dir.path().join("Zulu.txt"), "")?;
            fs::write(temp_dir.path().join("alpha.txt"), "")?;
            Ok(())
        },
        "",
        None,
    )
    .await;

    // Verify sorting: dirs first (case-insensitive), then files (case-insensitive).
    assert_eq!(output.entry_count, 4);
    assert_eq!(output.entries[0].name, "apple");
    assert_eq!(output.entries[0].kind, crate::EntryKind::Dir);
    assert_eq!(output.entries[1].name, "Zebra");
    assert_eq!(output.entries[1].kind, crate::EntryKind::Dir);
    assert_eq!(output.entries[2].name, "alpha.txt");
    assert_eq!(output.entries[2].kind, crate::EntryKind::File);
    assert_eq!(output.entries[3].name, "Zulu.txt");
    assert_eq!(output.entries[3].kind, crate::EntryKind::File);
}
