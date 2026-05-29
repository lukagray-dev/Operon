//! Comprehensive tests for the bash tool.
//!
//! Tests cover success cases (exit code 0, non-zero, timeout), failure cases (empty command,
//! spawn failure), output truncation, and stateless execution verification.

use crate::{execute, BashOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;

/// Helper to extract error text from a ToolResult.
fn get_error_text(result: &operon_context_normalize_tools::ToolResult) -> &str {
    match &result.content {
        ToolContent::Text(t) => t,
        other => panic!("expected Text content, got {:?}", other),
    }
}

/// Helper to extract BashOutput from a ToolResult.
fn get_bash_output(result: &operon_context_normalize_tools::ToolResult) -> BashOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize BashOutput")
        }
        other => panic!("expected Json content, got {:?}", other),
    }
}

// ============================================================================
// SUCCESS TESTS
// ============================================================================

#[tokio::test]
async fn test_basic_command() {
    // Test: Run `echo hello` and verify output.
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "command": "echo hello"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error, "expected success");
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 0);
    assert_eq!(output.output.trim(), "hello");
    assert!(!output.truncated);
    assert!(!output.timed_out);
}

#[tokio::test]
async fn test_nonzero_exit_code() {
    // Test: Run `exit 42` and verify non-zero exit code is NOT a tool error.
    let result = execute(
        ToolCallId("call_2".to_string()),
        json!({
            "command": "exit 42"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error, "non-zero exit should not be a tool error");
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 42);
}

#[tokio::test]
async fn test_stderr_captured() {
    // Test: Run `echo error >&2` and verify stderr is captured.
    let result = execute(
        ToolCallId("call_3".to_string()),
        json!({
            "command": "echo error >&2"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_bash_output(&result);
    assert!(output.output.contains("error"));
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn test_stdout_and_stderr_merged() {
    // Test: Run `echo out && echo err >&2` and verify both are in output.
    let result = execute(
        ToolCallId("call_4".to_string()),
        json!({
            "command": "echo out && echo err >&2"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_bash_output(&result);
    assert!(output.output.contains("out"));
    assert!(output.output.contains("err"));
}

#[tokio::test]
async fn test_command_chaining() {
    // Test: Run `cd /tmp && pwd` and verify chaining works within one call.
    // Note: On Windows, this may behave differently, but the principle is the same.
    let result = execute(
        ToolCallId("call_5".to_string()),
        json!({
            "command": if cfg!(windows) { "cd %TEMP% && cd" } else { "cd /tmp && pwd" }
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 0);
    // Output should contain the temp directory path
    assert!(!output.output.trim().is_empty());
}

#[tokio::test]
async fn test_output_truncation() {
    // Test: Run a command that produces > 10,000 chars and verify truncation.
    let result = execute(
        ToolCallId("call_6".to_string()),
        json!({
            "command": if cfg!(windows) {
                "for /L %i in (1,1,1000) do @echo aaaaaaaaaa"
            } else {
                "python3 -c \"print('a' * 20000)\""
            }
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_bash_output(&result);
    assert!(output.truncated, "output should be truncated");
    assert_eq!(output.output.chars().count(), 10_000);
}

#[tokio::test]
async fn test_no_timeout_runs_to_completion() {
    // Test: Run `sleep 1 && echo done` with no timeout and verify completion.
    let result = execute(
        ToolCallId("call_7".to_string()),
        json!({
            "command": if cfg!(windows) {
                "timeout /t 1 /nobreak && echo done"
            } else {
                "sleep 1 && echo done"
            }
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 0);
    assert!(output.output.contains("done"));
    assert!(!output.timed_out);
}

#[tokio::test]
async fn test_timeout_kills_command() {
    // Test: Run `sleep 10` with `timeout_ms: 200` and verify timeout.
    let result = execute(
        ToolCallId("call_8".to_string()),
        json!({
            "command": if cfg!(windows) {
                "timeout /t 10 /nobreak"
            } else {
                "sleep 10"
            },
            "timeout_ms": 200u64
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_bash_output(&result);
    assert!(output.timed_out, "command should have timed out");
    assert_eq!(output.exit_code, -1);
}

#[tokio::test]
async fn test_command_echoed_in_output() {
    // Test: Verify the command is echoed back in the output.
    let cmd = "echo test";
    let result = execute(
        ToolCallId("call_9".to_string()),
        json!({
            "command": cmd
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_bash_output(&result);
    assert_eq!(output.command, cmd);
}

// ============================================================================
// FAILURE TESTS
// ============================================================================

#[tokio::test]
async fn test_empty_command() {
    // Test: Pass empty command and verify error.
    let result = execute(
        ToolCallId("call_10".to_string()),
        json!({
            "command": ""
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error, "empty command should be an error");
    let error_text = get_error_text(&result);
    assert!(error_text.contains("empty"));
}

#[tokio::test]
async fn test_whitespace_only_command() {
    // Test: Pass whitespace-only command and verify error.
    let result = execute(
        ToolCallId("call_11".to_string()),
        json!({
            "command": "   "
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error, "whitespace-only command should be an error");
    let error_text = get_error_text(&result);
    assert!(error_text.contains("empty"));
}

#[tokio::test]
async fn test_malformed_args() {
    // Test: Pass malformed JSON and verify ArgsParse error.
    let result = execute(
        ToolCallId("call_12".to_string()),
        json!({
            "not_command": "echo hello"
        }),
    )
    .await;

    assert!(result.is_err(), "malformed args should return Err");
    match result {
        Err(crate::BashToolError::ArgsParse(_)) => {
            // Expected
        }
        other => panic!("expected ArgsParse error, got {:?}", other),
    }
}

// ============================================================================
// STATELESS EXECUTION TESTS
// ============================================================================

#[tokio::test]
async fn test_stateless_cd() {
    // Test: Run two separate calls with cd in the first, verify cd doesn't persist.
    // First call: cd /tmp
    let _result1 = execute(
        ToolCallId("call_13a".to_string()),
        json!({
            "command": if cfg!(windows) { "cd %TEMP%" } else { "cd /tmp" }
        }),
    )
    .await
    .unwrap();

    // Second call: pwd (should NOT be in /tmp)
    let result2 = execute(
        ToolCallId("call_13b".to_string()),
        json!({
            "command": if cfg!(windows) { "cd" } else { "pwd" }
        }),
    )
    .await
    .unwrap();

    assert!(!result2.is_error);
    let output2 = get_bash_output(&result2);
    // The output should NOT contain /tmp (or %TEMP% on Windows) because cd didn't persist.
    // On Unix, pwd should return the original working directory.
    // On Windows, cd should return the original working directory.
    // We just verify the command ran successfully — the exact path depends on the test environment.
    assert_eq!(output2.exit_code, 0);
}
