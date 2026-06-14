// tests.rs — Comprehensive tests for the bash tool.

use crate::execute;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use serde_json::json;
use tempfile::TempDir;

/// Extract plain-text content from a ToolResult.
fn extract_text(result: &ToolResult) -> String {
    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected ToolContent::Text, got {:?}", other),
    }
}

/// Returns the OS temp directory path as a String.
fn temp_dir_str() -> String {
    std::env::temp_dir()
        .to_str()
        .expect("temp dir path is not valid UTF-8")
        .to_string()
}

#[tokio::test]
async fn test_basic_command() {
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "path": temp_dir_str(),
            "command": "echo hello"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("exit: 0"));
    assert!(text.contains("hello"));
}

#[tokio::test]
async fn test_nonzero_exit_code() {
    let result = execute(
        ToolCallId("call_2".to_string()),
        json!({
            "path": temp_dir_str(),
            "command": "exit 42"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("exit: 42"));
}

#[tokio::test]
async fn test_stderr_captured() {
    let result = execute(
        ToolCallId("call_3".to_string()),
        json!({
            "path": temp_dir_str(),
            "command": if cfg!(windows) { "echo error 1>&2" } else { "echo error >&2" }
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("error"));
}

#[tokio::test]
async fn test_cwd_respected_by_subprocess() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_str().unwrap().to_string();

    let result = execute(
        ToolCallId("call_5b".to_string()),
        json!({
            "path": &path,
            "command": if cfg!(windows) { "cd" } else { "pwd" }
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("exit: 0"));
    let printed = text.trim();
    let dir_name = tmp.path().file_name().unwrap().to_str().unwrap();
    assert!(printed.contains(dir_name));
}

#[tokio::test]
async fn test_output_truncation() {
    let result = execute(
        ToolCallId("call_6".to_string()),
        json!({
            "path": temp_dir_str(),
            "command": if cfg!(windows) {
                "for /L %i in (1,1,500) do @echo aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            } else {
                "python3 -c \"print('a' * 20000)\""
            }
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("***truncated***"));
}

#[tokio::test]
async fn test_timeout_kills_command() {
    let result = execute(
        ToolCallId("call_8".to_string()),
        json!({
            "path": temp_dir_str(),
            "command": if cfg!(windows) { "ping -n 30 127.0.0.1" } else { "sleep 10" },
            "timeout": "300"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("***timed out***"));
    assert!(text.contains("exit: -1"));
}

#[tokio::test]
async fn test_empty_command() {
    let result = execute(
        ToolCallId("call_10".to_string()),
        json!({
            "path": temp_dir_str(),
            "command": ""
        }),
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_cwd_does_not_exist() {
    let nonexistent = if cfg!(windows) {
        "C:\\this_path_does_not_exist_operon_9999\\sub"
    } else {
        "/this/path/does/not/exist/at/all/9999"
    };

    let result = execute(
        ToolCallId("call_14".to_string()),
        json!({
            "path": nonexistent,
            "command": "echo hello"
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("does not exist"));
}
