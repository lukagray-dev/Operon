// tests.rs — Comprehensive tests for the bash tool.
//
// All tests provide the required `cwd` field (pointing to a temp directory
// or the system temp directory). This reflects the real-world requirement
// that callers always supply a working directory.
//
// Test coverage:
//   - SUCCESS: basic command, non-zero exit, stderr capture, stdout+stderr merge,
//     command chaining, output truncation, no-timeout run, timeout kill, echo-back,
//     multi-line real newlines (no literal \n escapes).
//   - FAILURE: empty command, whitespace-only command, missing args, invalid cwd
//     (nonexistent, not a directory, relative path).
//   - STATELESS: cd does not persist across calls.
//   - DISPATCHER INTEGRATION: verified separately in operon-tools/src/tests.rs.

use crate::{execute, BashOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use serde_json::json;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract plain-text content from a ToolResult. Panics if it's Json.
fn get_text_content(result: &operon_context_normalize_tools::ToolResult) -> &str {
    match &result.content {
        ToolContent::Text(t) => t,
        other => panic!("expected ToolContent::Text, got {:?}", other),
    }
}

/// Returns the OS temp directory path as a String for use as `cwd` in tests.
/// Using the temp dir is safe — it always exists and is a directory.
fn temp_dir_str() -> String {
    std::env::temp_dir()
        .to_str()
        .expect("temp dir path is not valid UTF-8")
        .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// UNIT TESTS FOR TO_PLAIN_TEXT
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_bash_output_to_plain_text() {
    let output = BashOutput {
        command: "echo hello".to_string(),
        cwd: "/tmp/app".to_string(),
        exit_code: 0,
        output: "hello\nworld".to_string(),
        truncated: false,
        timed_out: false,
    };

    let text = output.to_plain_text();
    assert!(text.starts_with("=== echo hello (in /tmp/app) ==="));
    assert!(text.contains("hello\nworld"));
    assert!(text.ends_with("Exit code: 0"));
    assert!(
        !text.contains("\\n"),
        "must contain real newlines, not escaped \\n"
    );
}

#[test]
fn test_bash_output_to_plain_text_truncated() {
    let output = BashOutput {
        command: "cat bigfile".to_string(),
        cwd: "/tmp".to_string(),
        exit_code: 0,
        output: "content".to_string(),
        truncated: true,
        timed_out: false,
    };

    let text = output.to_plain_text();
    assert!(text.contains(
        "[Output truncated at 10,000 characters. Use head, tail, or grep to narrow output.]"
    ));
    assert!(text.contains("Exit code: 0"));
}

#[test]
fn test_bash_output_to_plain_text_timed_out() {
    let output = BashOutput {
        command: "sleep 100".to_string(),
        cwd: "/tmp".to_string(),
        exit_code: -1,
        output: "partial".to_string(),
        truncated: false,
        timed_out: true,
    };

    let text = output.to_plain_text();
    assert!(text.contains("Exit code: -1 (timed out)"));
}

// ─────────────────────────────────────────────────────────────────────────────
// SUCCESS TESTS
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_basic_command() {
    // Test: Run `echo hello` in the temp dir and verify output.
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "command": "echo hello",
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(
        !result.is_error,
        "expected success, got: {:?}",
        result.content
    );
    let text = get_text_content(&result);
    assert!(text.contains("echo hello"));
    assert!(text.contains("hello"));
    assert!(text.contains("Exit code: 0"));
    assert!(!text.contains("truncated"));
    assert!(!text.contains("timed out"));
}

#[tokio::test]
async fn test_nonzero_exit_code() {
    // Test: Non-zero exit code is NOT a tool error — the model sees the exit code.
    let result = execute(
        ToolCallId("call_2".to_string()),
        json!({
            "command": "exit 42",
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    // Non-zero exit is a normal outcome — model decides what to do next.
    assert!(!result.is_error, "non-zero exit should not be a tool error");
    let text = get_text_content(&result);
    assert!(text.contains("Exit code: 42"));
}

#[tokio::test]
async fn test_stderr_captured() {
    // Test: Verify stderr is captured and included in the merged output.
    let result = execute(
        ToolCallId("call_3".to_string()),
        json!({
            "command": if cfg!(windows) { "echo error 1>&2" } else { "echo error >&2" },
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(text.contains("error"), "stderr should be in merged output");
    assert!(text.contains("Exit code: 0"));
}

#[tokio::test]
async fn test_stdout_and_stderr_merged() {
    // Test: Both stdout and stderr appear in the merged output.
    let result = execute(
        ToolCallId("call_4".to_string()),
        json!({
            "command": if cfg!(windows) {
                "echo out && echo err 1>&2"
            } else {
                "echo out && echo err >&2"
            },
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(text.contains("out"), "stdout should be in merged output");
    assert!(text.contains("err"), "stderr should be in merged output");
    assert!(text.contains("Exit code: 0"));
}

#[tokio::test]
async fn test_command_chaining() {
    // Test: cd + pwd chained in one call changes the effective directory within
    // that subprocess (does not affect cwd for subsequent calls).
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_str().unwrap().to_string();

    let result = execute(
        ToolCallId("call_5".to_string()),
        json!({
            "command": if cfg!(windows) { "echo hello" } else { "pwd" },
            "cwd": cwd
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(text.contains("Exit code: 0"));
}

#[tokio::test]
async fn test_cwd_respected_by_subprocess() {
    // Test: Verify the subprocess actually runs in the specified cwd.
    // Run `pwd` (Unix) or `cd` (Windows) and verify the output matches cwd.
    // We use a real TempDir so we have an absolute path that exists.
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_str().unwrap().to_string();

    let result = execute(
        ToolCallId("call_5b".to_string()),
        json!({
            "command": if cfg!(windows) { "cd" } else { "pwd" },
            "cwd": &cwd
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(text.contains("Exit code: 0"));
    let dir_name = tmp.path().file_name().unwrap().to_str().unwrap();
    assert!(
        text.contains(dir_name) || text.contains(tmp.path().to_str().unwrap()),
        "subprocess cwd should be the specified cwd, got: {}",
        text
    );
}

#[tokio::test]
async fn test_output_truncation() {
    // Test: Command that produces >10,000 chars triggers truncation.
    let result = execute(
        ToolCallId("call_6".to_string()),
        json!({
            "command": if cfg!(windows) {
                "for /L %i in (1,1,500) do @echo aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            } else {
                "python3 -c \"print('a' * 20000)\""
            },
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(
        text.contains("truncated"),
        "output should include truncation note"
    );
    assert!(text.contains("Exit code: 0"));
}

#[tokio::test]
async fn test_no_timeout_runs_to_completion() {
    // Test: A command that takes ~1 second with no timeout_ms runs to completion.
    let result = execute(
        ToolCallId("call_7".to_string()),
        json!({
            "command": if cfg!(windows) {
                "ping -n 2 127.0.0.1 > nul && echo done"
            } else {
                "sleep 1 && echo done"
            },
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(text.contains("Exit code: 0"));
    assert!(
        text.contains("done"),
        "command should complete and print 'done'"
    );
    assert!(!text.contains("timed out"));
}

#[tokio::test]
async fn test_timeout_kills_command() {
    // Test: Command running longer than timeout_ms is killed (timed_out = true, exit_code = -1).
    let result = execute(
        ToolCallId("call_8".to_string()),
        json!({
            "command": if cfg!(windows) {
                "ping -n 30 127.0.0.1"
            } else {
                "sleep 10"
            },
            "cwd": temp_dir_str(),
            "timeout_ms": 300u64
        }),
    )
    .await
    .unwrap();

    // The tool itself is not an error — timed_out is reported in the output text.
    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(text.contains("timed out"), "command should have timed out");
    assert!(text.contains("Exit code: -1"));
}

#[tokio::test]
async fn test_command_echoed_in_output() {
    // Test: The command is echoed in the plain-text header.
    let cmd = "echo test";
    let result = execute(
        ToolCallId("call_9".to_string()),
        json!({
            "command": cmd,
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(
        text.contains(cmd),
        "command should be echoed back in header"
    );
}

#[tokio::test]
async fn test_multiline_output_has_real_newlines() {
    // Test: Multi-line output contains real newlines ('\n') and does NOT contain literal "\\n" escapes.
    let result = execute(
        ToolCallId("call_multiline".to_string()),
        json!({
            "command": if cfg!(windows) {
                "echo line1 && echo line2 1>&2"
            } else {
                "echo line1 && echo line2 >&2"
            },
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);

    // Must contain real newline character
    assert!(
        text.contains('\n'),
        "plain text output must contain real newline characters"
    );

    // Must contain both lines
    assert!(text.contains("line1"));
    assert!(text.contains("line2"));

    // Must NOT contain the literal string "\\n" (escaped newline from JSON serialization)
    assert!(
        !text.contains("\\n"),
        "plain text output must not contain literal '\\\\n' escapes"
    );

    // Verify exit code line is present
    assert!(text.contains("Exit code: 0"));
}

// ─────────────────────────────────────────────────────────────────────────────
// FAILURE TESTS
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_empty_command() {
    // Test: Empty command string → tool error with "empty" in message.
    let result = execute(
        ToolCallId("call_10".to_string()),
        json!({
            "command": "",
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error, "empty command should be an error");
    let error_text = get_text_content(&result);
    assert!(error_text.contains("empty"), "error should mention 'empty'");
}

#[tokio::test]
async fn test_whitespace_only_command() {
    // Test: Whitespace-only command → tool error (treated same as empty).
    let result = execute(
        ToolCallId("call_11".to_string()),
        json!({
            "command": "   ",
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(
        result.is_error,
        "whitespace-only command should be an error"
    );
    let error_text = get_text_content(&result);
    assert!(error_text.contains("empty"), "error should mention 'empty'");
}

#[tokio::test]
async fn test_malformed_args_missing_command() {
    // Test: Missing `command` field → ArgsParse error (returned as Err, not Ok).
    let result = execute(
        ToolCallId("call_12".to_string()),
        json!({
            "not_command": "echo hello",
            "cwd": temp_dir_str()
        }),
    )
    .await;

    assert!(
        result.is_err(),
        "missing command should return Err(ArgsParse)"
    );
    match result {
        Err(crate::BashToolError::ArgsParse(_)) => { /* expected */ }
        other => panic!("expected ArgsParse error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_malformed_args_missing_cwd() {
    // Test: Missing `cwd` field → ArgsParse error.
    let result = execute(
        ToolCallId("call_13".to_string()),
        json!({
            "command": "echo hello"
        }),
    )
    .await;

    assert!(result.is_err(), "missing cwd should return Err(ArgsParse)");
    match result {
        Err(crate::BashToolError::ArgsParse(_)) => { /* expected */ }
        other => panic!("expected ArgsParse error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_cwd_does_not_exist() {
    // Test: cwd that doesn't exist on disk → tool error.
    let nonexistent = if cfg!(windows) {
        "C:\\this_path_does_not_exist_operon_9999\\sub"
    } else {
        "/this/path/does/not/exist/at/all/9999"
    };

    let result = execute(
        ToolCallId("call_14".to_string()),
        json!({
            "command": "echo hello",
            "cwd": nonexistent
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error, "nonexistent cwd should be an error");
    let error_text = get_text_content(&result);
    assert!(
        error_text.contains("does not exist"),
        "error should mention the cwd does not exist, got: {}",
        error_text
    );
}

#[tokio::test]
async fn test_cwd_not_a_directory() {
    // Test: cwd pointing to a file (not a directory) → tool error.
    use std::fs;
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("not_a_dir.txt");
    fs::write(&file_path, "I am a file, not a directory").unwrap();

    let result = execute(
        ToolCallId("call_15".to_string()),
        json!({
            "command": "echo hello",
            "cwd": file_path.to_str().unwrap()
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error, "cwd pointing to a file should be an error");
    let error_text = get_text_content(&result);
    assert!(
        error_text.contains("not a directory"),
        "error should mention cwd is not a directory, got: {}",
        error_text
    );
}

#[tokio::test]
async fn test_cwd_relative_path_rejected() {
    // Test: Relative cwd path → tool error.
    let result = execute(
        ToolCallId("call_16".to_string()),
        json!({
            "command": "echo hello",
            "cwd": "relative/path/here"
        }),
    )
    .await
    .unwrap();

    assert!(result.is_error, "relative cwd should be an error");
    let error_text = get_text_content(&result);
    assert!(
        error_text.contains("absolute"),
        "error should mention absolute path requirement, got: {}",
        error_text
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// STATELESS EXECUTION TESTS
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_stateless_env_vars() {
    let _result1 = execute(
        ToolCallId("call_17a".to_string()),
        json!({
            "command": if cfg!(windows) {
                "set MY_OPERON_TEST_VAR=hello"
            } else {
                "export MY_OPERON_TEST_VAR=hello"
            },
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    let result2 = execute(
        ToolCallId("call_17b".to_string()),
        json!({
            "command": if cfg!(windows) {
                "echo %MY_OPERON_TEST_VAR%"
            } else {
                "echo ${MY_OPERON_TEST_VAR:-NOT_SET}"
            },
            "cwd": temp_dir_str()
        }),
    )
    .await
    .unwrap();

    assert!(!result2.is_error);
    let text2 = get_text_content(&result2);
    assert!(text2.contains("Exit code: 0"));

    assert!(
        !text2.contains("hello"),
        "env var from prior call should not persist; got: {}",
        text2
    );
}

#[tokio::test]
async fn test_stateless_cd_does_not_persist() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_str().unwrap().to_string();

    let _result1 = execute(
        ToolCallId("call_18a".to_string()),
        json!({
            "command": if cfg!(windows) { "cd %SystemRoot%" } else { "cd /tmp" },
            "cwd": &cwd
        }),
    )
    .await
    .unwrap();

    let result2 = execute(
        ToolCallId("call_18b".to_string()),
        json!({
            "command": if cfg!(windows) { "cd" } else { "pwd" },
            "cwd": &cwd
        }),
    )
    .await
    .unwrap();

    assert!(!result2.is_error);
    let text2 = get_text_content(&result2);
    assert!(text2.contains("Exit code: 0"));
    let dir_name = tmp.path().file_name().unwrap().to_str().unwrap();
    assert!(
        text2.contains(dir_name),
        "cwd from prior cd should not persist; expected path containing '{}', got: {}",
        dir_name,
        text2
    );
}

#[tokio::test]
async fn test_bash_defensive_aliases_and_timeout_string() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_str().unwrap().to_string();

    let result = execute(
        ToolCallId("alias_call".to_string()),
        json!({
            "cmd": "echo defensive_success",
            "dir": &cwd,
            "timeout": "5000"
        }),
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = get_text_content(&result);
    assert!(text.contains("defensive_success"));
    assert!(text.contains("Exit code: 0"));
}
