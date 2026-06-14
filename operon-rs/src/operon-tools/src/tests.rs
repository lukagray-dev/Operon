/// Tests for the tool dispatcher.
///
/// These tests verify:
/// - Unknown tool handling
/// - Malformed args detection and degradation
/// - Tiered description switching
/// - Successful dispatch without degradation
use crate::dispatcher::Dispatcher;
use crate::{ToolProgress, ToolProgressEmitter, ToolProgressStage};
use operon_context_normalize::tools::{ToolCall, ToolCallId, ToolContent};
use serde_json::json;

/// Helper to create a ToolCall for testing.
fn make_call(name: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: ToolCallId(format!("call_{}", name)),
        name: name.to_string(),
        arguments,
    }
}

#[tokio::test]
async fn test_unknown_tool_returns_error_result() {
    let mut d = Dispatcher::new();
    d.register_fs_tools();

    let result = d.dispatch(make_call("nonexistent_tool", json!({}))).await;

    assert!(result.is_error);
    assert!(result.name == "nonexistent_tool");
}



#[tokio::test]
async fn test_lazy_loading_tools() {
    let mut d = Dispatcher::new();

    // We register the load_tools meta-tool (belongs to the "core" group)
    // and the filesystem tools (belong to the "fs" group).
    d.register_load_tool();
    d.register_fs_tools();

    // 1. Initially, only the bootstrap tools (in the "core" group) should be returned
    //    by definitions(). Other tool groups (like "fs") are hidden to save token size!
    let defs: Vec<_> = d.definitions().collect();
    assert_eq!(
        defs.len(),
        1,
        "Initially, only load_tools should be visible to the AI model"
    );
    assert_eq!(
        defs[0].name, "load_tools",
        "The visible tool must be load_tools"
    );

    // 2. The AI model requests loading the "fs" group.
    let result = d
        .dispatch(make_call("load_tools", json!({ "group": "fs" })))
        .await;
    assert!(
        !result.is_error,
        "The load_tools execution should complete successfully"
    );

    // 3. Verify that the dispatcher successfully tracked that the "fs" group is now loaded.
    assert!(
        d.loaded_groups().contains("fs"),
        "The 'fs' group should be marked as loaded in the dispatcher"
    );

    // 4. Now, the next time definitions() is called, it should yield load_tools AND all the fs tools.
    let defs_after: Vec<_> = d.definitions().collect();
    // The "fs" group has 7 tools (read, grep, ls, edit, write, append, delete).
    // Plus the 1 bootstrap tool (load_tools). Total is 8 tools.
    assert_eq!(
        defs_after.len(),
        8,
        "We expect 8 tools to be visible now that the fs group is unlocked (7 fs + 1 core)"
    );
    assert!(
        defs_after.iter().any(|def| def.name == "read"),
        "The definitions must now contain the 'read' tool"
    );
}



#[tokio::test]
async fn test_successful_dispatch_does_not_degrade() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let file = dir.path().join("hello.txt");
    fs::write(&file, "hello world\n").unwrap();

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    let result = d
        .dispatch(make_call(
            "read",
            json!({ "paths": file.to_str().unwrap() }),
        ))
        .await;

    assert!(!result.is_error, "Expected success, got error: {:?}", result.content);
}

// ============================================================================
// Read-before-write/edit enforcement tests
// ============================================================================

#[tokio::test]
async fn test_write_existing_file_blocked_without_read() {
    use std::fs;
    use tempfile::NamedTempFile;

    // Create a real temp file on disk with some content
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    fs::write(path, "original content\n").unwrap();

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    // Dispatch a write call for that path without first dispatching a read
    let result = d
        .dispatch(make_call(
            "write",
            json!({
                "path": path.to_str().unwrap(),
                "__body__": "new content\n"
            }),
        ))
        .await;

    assert!(result.is_error, "write to existing file should be blocked");
    let error_text = match &result.content {
        ToolContent::Text(t) => t,
        _ => panic!("expected Text content"),
    };
    assert!(
        error_text.contains("read-before-write"),
        "error should mention read-before-write enforcement"
    );
}

#[tokio::test]
async fn test_write_new_file_allowed_without_read() {
    use tempfile::TempDir;

    // Pick a path that does NOT exist on disk
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nonexistent_file.txt");

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    // Dispatch write without a prior read
    let result = d
        .dispatch(make_call(
            "write",
            json!({
                "path": path.to_str().unwrap(),
                "content": "new file content\n"
            }),
        ))
        .await;

    // New file creation should be allowed (exempt from read-before-write)
    assert!(
        !result.is_error,
        "write to new file should be allowed without prior read"
    );
}

#[tokio::test]
async fn test_edit_blocked_without_read() {
    use std::fs;
    use tempfile::NamedTempFile;

    // Create a real temp file with content
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    fs::write(path, "fn foo() {}\n").unwrap();

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    // Dispatch edit without a prior read
    let result = d
        .dispatch(make_call(
            "edit",
            json!({
                "path": path.to_str().unwrap(),
                "__body__": "@@\n-fn foo() {}\n+fn bar() {}"
            }),
        ))
        .await;

    assert!(result.is_error, "edit should be blocked without prior read");
    let error_text = match &result.content {
        ToolContent::Text(t) => t,
        _ => panic!("expected Text content"),
    };
    assert!(
        error_text.contains("read-before-edit"),
        "error should mention read-before-edit enforcement"
    );
}

#[tokio::test]
async fn test_read_then_edit_allowed() {
    use std::fs;
    use tempfile::NamedTempFile;

    // Create a real temp file with content
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    fs::write(path, "fn foo() {}\n").unwrap();

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    // Dispatch read first
    let read_result = d
        .dispatch(make_call(
            "read",
            json!({ "paths": path.to_str().unwrap() }),
        ))
        .await;

    assert!(!read_result.is_error, "read should succeed, but got error: {:?}", read_result.content);

    // Now dispatch edit — should be allowed
    let edit_result = d
        .dispatch(make_call(
            "edit",
            json!({
                "path": path.to_str().unwrap(),
                "__body__": "@@\n-fn foo() {}\n+fn bar() {}"
            }),
        ))
        .await;

    assert!(
        !edit_result.is_error,
        "edit should be allowed after read: {}",
        match &edit_result.content {
            ToolContent::Text(t) => t,
            _ => "unknown error",
        }
    );

    // Verify the path is in the ledger
    assert!(
        d.read_ledger().has_been_read(path),
        "path should be recorded in ledger after read"
    );
}

#[tokio::test]
async fn test_read_then_write_allowed() {
    use std::fs;
    use tempfile::NamedTempFile;

    // Create a real temp file
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    fs::write(path, "original content\n").unwrap();

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    // Dispatch read first
    let read_result = d
        .dispatch(make_call(
            "read",
            json!({ "paths": path.to_str().unwrap() }),
        ))
        .await;

    assert!(!read_result.is_error, "read should succeed, but got error: {:?}", read_result.content);

    // Now dispatch write — should be allowed
    let write_result = d
        .dispatch(make_call(
            "write",
            json!({
                "path": path.to_str().unwrap(),
                "__body__": "new content\n"
            }),
        ))
        .await;

    assert!(
        !write_result.is_error,
        "write should be allowed after read: {}",
        match &write_result.content {
            ToolContent::Text(t) => t,
            _ => "unknown error",
        }
    );
}

#[tokio::test]
async fn test_compaction_clears_ledger() {
    use std::fs;
    use tempfile::NamedTempFile;

    // Create a real temp file
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    fs::write(path, "fn foo() {}\n").unwrap();

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    // Dispatch read — path now in ledger
    let read_result = d
        .dispatch(make_call(
            "read",
            json!({ "paths": path.to_str().unwrap() }),
        ))
        .await;

    assert!(!read_result.is_error, "read should succeed, but got error: {:?}", read_result.content);
    assert!(
        d.read_ledger().has_been_read(path),
        "path should be in ledger after read"
    );

    // Call notify_compaction
    d.notify_compaction();

    // Ledger should be cleared
    assert!(
        d.read_ledger().is_empty(),
        "ledger should be empty after compaction"
    );

    // Dispatch edit — should now be blocked (ledger was cleared)
    let edit_result = d
        .dispatch(make_call(
            "edit",
            json!({
                "path": path.to_str().unwrap(),
                "__body__": "@@\n-fn foo() {}\n+fn bar() {}"
            }),
        ))
        .await;

    assert!(
        edit_result.is_error,
        "edit should be blocked after compaction cleared the ledger"
    );
}

#[tokio::test]
async fn test_failed_read_does_not_record() {
    use tempfile::TempDir;

    // Dispatch read on a path that doesn't exist
    let dir = TempDir::new().unwrap();
    let nonexistent_path = dir.path().join("does_not_exist.txt");

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    let read_result = d
        .dispatch(make_call(
            "read",
            json!({ "paths": nonexistent_path.to_str().unwrap() }),
        ))
        .await;

    // read returns per-file errors, not top-level errors
    assert!(!read_result.is_error, "read tool returns per-file errors, but got error: {:?}", read_result.content);

    // The nonexistent path should NOT be in the ledger
    assert!(
        !d.read_ledger().has_been_read(&nonexistent_path),
        "failed read should not record path in ledger"
    );

    // Dispatch edit on that path — should be blocked
    let edit_result = d
        .dispatch(make_call(
            "edit",
            json!({
                "path": nonexistent_path.to_str().unwrap(),
                "__body__": "@@\n-anything\n+something"
            }),
        ))
        .await;

    assert!(
        edit_result.is_error,
        "edit should be blocked for path that failed to read"
    );
}

// ============================================================================
// SHELL TOOLS TESTS
// ============================================================================

#[tokio::test]
async fn test_bash_tool_registration_and_dispatch() {
    // Test: Verify bash tool can be registered and dispatched successfully.
    // path is required by BashArgs (Option C policy enforcement).
    let mut d = Dispatcher::new();
    d.register_shell_tools();

    // Dispatch a simple bash command with the required path field.
    let result = d
        .dispatch(make_call(
            "bash",
            json!({
                "path": std::env::temp_dir().to_str().unwrap(),
                "command": "echo hello"
            }),
        ))
        .await;

    assert!(!result.is_error, "bash tool should execute successfully: {:?}", result.content);
    assert_eq!(result.name, "bash");

    // Verify the result contains plain-text output with exit code and hello.
    match &result.content {
        ToolContent::Text(text) => {
            assert!(text.contains("exit: 0"), "expected exit: 0, got: {}", text);
            assert!(text.contains("hello"), "expected hello, got: {}", text);
        }
        other => panic!("expected Text content, got {:?}", other),
    }
}



#[tokio::test]
async fn test_bash_tool_empty_command_error() {
    // Test: Empty command with valid path → tool-level error (not ArgsParse).
    // path is present and valid, so deserialization succeeds. The executor
    // catches the empty command and returns is_error=true.
    let mut d = Dispatcher::new();
    d.register_shell_tools();

    let result = d
        .dispatch(make_call(
            "bash",
            json!({
                "path": std::env::temp_dir().to_str().unwrap(),
                "command": ""
            }),
        ))
        .await;

    assert!(result.is_error, "expected error, got: {:?}", result.content);
    match &result.content {
        ToolContent::Text(t) => {
            assert!(t.contains("empty"), "error message should mention 'empty', got: {}", t);
        }
        other => panic!("expected Text content, got {:?}", other),
    }
}

#[tokio::test]
async fn test_bash_tool_nonzero_exit_not_error() {
    // Test: Non-zero exit code is a normal outcome — not a tool error.
    // The model receives exit_code and decides how to respond.
    let mut d = Dispatcher::new();
    d.register_shell_tools();

    let result = d
        .dispatch(make_call(
            "bash",
            json!({
                "path": std::env::temp_dir().to_str().unwrap(),
                "command": "exit 42"
            }),
        ))
        .await;

    // Non-zero exit should NOT be a tool error.
    assert!(!result.is_error, "non-zero exit should not be a tool error: {:?}", result.content);

    match &result.content {
        ToolContent::Text(text) => {
            assert!(text.contains("exit: 42"), "expected exit: 42, got: {}", text);
        }
        other => panic!("expected Text content, got {:?}", other),
    }
}

#[tokio::test]
async fn test_progress_events_flow_through_dispatcher() {
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("progress_write.txt");

    let seen: Arc<Mutex<Vec<ToolProgress>>> = Arc::new(Mutex::new(Vec::new()));
    let emitter: ToolProgressEmitter = {
        let seen = Arc::clone(&seen);
        Arc::new(move |progress: ToolProgress| {
            seen.lock().unwrap().push(progress);
        })
    };

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    let result = d
        .dispatch_with_progress(
            make_call(
                "write",
                json!({
                    "path": path.to_str().unwrap(),
                    "__body__": "hello progress\n"
                }),
            ),
            Some(emitter),
        )
        .await;

    assert!(!result.result.is_error, "write should succeed");

    let events = seen.lock().unwrap().clone();
    let stages: Vec<_> = events.iter().map(|event| &event.stage).collect();
    assert_eq!(
        stages,
        vec![
            &ToolProgressStage::Started,
            &ToolProgressStage::Running,
            &ToolProgressStage::Completed,
        ]
    );
    assert_eq!(events[1].target.as_deref(), Some(path.to_str().unwrap()));
    assert!(
        events[1].message.contains("Writing"),
        "running update should describe the active file write"
    );
}
