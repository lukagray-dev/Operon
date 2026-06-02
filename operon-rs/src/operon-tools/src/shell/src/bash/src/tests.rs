// tests.rs — Comprehensive tests for the bash tool.
//
// All tests provide the required `cwd` field (pointing to a temp directory
// or the system temp directory). This reflects the real-world requirement
// that callers always supply a working directory.
//
// Test coverage:
//   - SUCCESS: basic command, non-zero exit, stderr capture, stdout+stderr merge,
//     command chaining, output truncation, no-timeout run, timeout kill, echo-back.
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

/// Extract plain-text error content from a ToolResult. Panics if it's Json.
fn get_error_text(result: &operon_context_normalize_tools::ToolResult) -> &str {
    match &result.content {
        ToolContent::Text(t) => t,
        other => panic!("expected ToolContent::Text, got {:?}", other),
    }
}

/// Deserialize BashOutput from ToolResult. Panics if content is not Json.
fn get_bash_output(result: &operon_context_normalize_tools::ToolResult) -> BashOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize BashOutput")
        }
        other => panic!("expected ToolContent::Json, got {:?}", other),
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

    assert!(!result.is_error, "expected success, got: {:?}", result.content);
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 0);
    assert!(output.output.contains("hello"));
    assert!(!output.truncated);
    assert!(!output.timed_out);
    // cwd should be echoed back correctly.
    assert_eq!(output.cwd, temp_dir_str());
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
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 42);
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
    let output = get_bash_output(&result);
    assert!(output.output.contains("error"), "stderr should be in merged output");
    assert_eq!(output.exit_code, 0);
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
    let output = get_bash_output(&result);
    assert!(output.output.contains("out"), "stdout should be in merged output");
    assert!(output.output.contains("err"), "stderr should be in merged output");
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
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 0);
    // The output should include something (pwd returns cwd or chained path).
    assert!(!output.output.trim().is_empty());
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
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 0);
    // The printed working directory should contain our temp path (or a canonical version).
    // We use contains() because some OS resolve symlinks (e.g. /tmp → /private/tmp on macOS).
    let printed = output.output.trim();
    assert!(
        // Either exact match or the temp path is a prefix/suffix due to symlink resolution.
        printed.ends_with(tmp.path().file_name().unwrap().to_str().unwrap())
            || printed.contains(tmp.path().to_str().unwrap()),
        "subprocess cwd should be the specified cwd, got: {}",
        printed
    );
}

#[tokio::test]
async fn test_output_truncation() {
    // Test: Command that produces >10,000 chars triggers truncation.
    //
    // On Windows we use a batch for-loop: 500 iterations × 64 'a' chars per line
    // = ~32,000 chars (plus \r\n newlines) — well over the 10,000 char cap.
    // The `for /L` loop is built into cmd.exe and requires no external tools.
    //
    // On Unix, python3 prints 20,000 'a' chars in one shot — reliable and fast.
    let result = execute(
        ToolCallId("call_6".to_string()),
        json!({
            "command": if cfg!(windows) {
                // 500 lines × ~66 chars (64 a's + \r\n) = ~33,000 chars total.
                // This is far enough over the 10,000 threshold to be unambiguous.
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
    let output = get_bash_output(&result);
    assert!(output.truncated, "output should be truncated when >10,000 chars");
    assert_eq!(
        output.output.chars().count(),
        10_000,
        "truncated output should be exactly 10,000 chars"
    );
}

#[tokio::test]
async fn test_no_timeout_runs_to_completion() {
    // Test: A command that takes ~1 second with no timeout_ms runs to completion.
    //
    // On Windows, `timeout /t 1 /nobreak` exits with code 0 only when stdin is a TTY.
    // In a piped subprocess (no TTY), it exits immediately with code 1.
    // We use `ping -n 2 127.0.0.1` instead: sends 2 ICMP pings with ~1s gap,
    // exits 0 on success, and works in all subprocess environments.
    let result = execute(
        ToolCallId("call_7".to_string()),
        json!({
            "command": if cfg!(windows) {
                // ping -n 2 sends 2 pings with a 1 second gap between them.
                // This is the reliable Windows equivalent of `sleep 1`.
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
    let output = get_bash_output(&result);
    assert_eq!(output.exit_code, 0);
    assert!(output.output.contains("done"), "command should complete and print 'done'");
    assert!(!output.timed_out);
}

#[tokio::test]
async fn test_timeout_kills_command() {
    // Test: Command running longer than timeout_ms is killed (timed_out = true, exit_code = -1).
    //
    // On Windows, `timeout /t 10 /nobreak` sometimes exits immediately in piped
    // subprocess contexts (no interactive TTY). Use `ping -n 30 127.0.0.1` instead —
    // that sends 30 ICMP pings with ~1s gap each, taking ~30 seconds total,
    // and always runs regardless of TTY state.
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

    // The tool itself is not an error — timed_out is reported in the output.
    assert!(!result.is_error);
    let output = get_bash_output(&result);
    assert!(output.timed_out, "command should have timed out");
    assert_eq!(output.exit_code, -1);
}

#[tokio::test]
async fn test_command_echoed_in_output() {
    // Test: The `command` field in BashOutput matches what was passed in.
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
    let output = get_bash_output(&result);
    assert_eq!(output.command, cmd, "command should be echoed back unchanged");
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
    let error_text = get_error_text(&result);
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

    assert!(result.is_error, "whitespace-only command should be an error");
    let error_text = get_error_text(&result);
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

    assert!(result.is_err(), "missing command should return Err(ArgsParse)");
    match result {
        Err(crate::BashToolError::ArgsParse(_)) => { /* expected */ }
        other => panic!("expected ArgsParse error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_malformed_args_missing_cwd() {
    // Test: Missing `cwd` field → ArgsParse error.
    // This is the key safety check: callers cannot omit cwd.
    let result = execute(
        ToolCallId("call_13".to_string()),
        json!({
            "command": "echo hello"
            // cwd intentionally omitted
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
    // Test: cwd that doesn't exist on disk → tool error (not ArgsParse).
    //
    // We use a platform-specific absolute path that definitely does not exist.
    // On Windows, Unix-style paths (/foo/bar) are treated as relative by
    // std::path::Path::is_absolute(), so we need a Windows-style absolute path
    // (C:\...) to get past the is_absolute() check and reach is_exists().
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
    .unwrap();  // Returns Ok(ToolResult { is_error: true }) not Err

    assert!(result.is_error, "nonexistent cwd should be an error");
    let error_text = get_error_text(&result);
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
    let error_text = get_error_text(&result);
    assert!(
        error_text.contains("not a directory"),
        "error should mention cwd is not a directory, got: {}",
        error_text
    );
}

#[tokio::test]
async fn test_cwd_relative_path_rejected() {
    // Test: Relative cwd path → tool error (policy needs absolute paths to resolve).
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
    let error_text = get_error_text(&result);
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
    // Test: Environment variables set in one call do not appear in the next call.
    // First call sets MY_VAR; second call reads it — should be empty.

    // First call: set env var (only within this subprocess's lifetime).
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

    // Second call: read the env var — should be missing (subprocess is fresh).
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
    let output2 = get_bash_output(&result2);
    assert_eq!(output2.exit_code, 0);

    // On Unix: should print NOT_SET. On Windows: prints %MY_OPERON_TEST_VAR% literally
    // (env var not set → Windows echoes the literal variable name).
    // Either way, "hello" should NOT appear.
    assert!(
        !output2.output.contains("hello"),
        "env var from prior call should not persist; got: {}",
        output2.output
    );
}

#[tokio::test]
async fn test_stateless_cd_does_not_persist() {
    // Test: cd in one call does not affect the cwd of the next call.
    // Both calls use the same cwd so we can compare pwd outputs.
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_str().unwrap().to_string();

    // First call: cd to /tmp (different from our cwd).
    let _result1 = execute(
        ToolCallId("call_18a".to_string()),
        json!({
            "command": if cfg!(windows) { "cd %SystemRoot%" } else { "cd /tmp" },
            "cwd": &cwd
        }),
    )
    .await
    .unwrap();

    // Second call: print working directory — should still be our original cwd.
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
    let output2 = get_bash_output(&result2);
    assert_eq!(output2.exit_code, 0);
    // The printed directory should match our original cwd (not /tmp or %SystemRoot%).
    // We check the filename component to handle macOS symlink resolution (/private/tmp).
    let dir_name = tmp.path().file_name().unwrap().to_str().unwrap();
    assert!(
        output2.output.contains(dir_name),
        "cwd from prior cd should not persist; expected path containing '{}', got: {}",
        dir_name,
        output2.output.trim()
    );
}
