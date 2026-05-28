/// Tests for the tool dispatcher.
///
/// These tests verify:
/// - Unknown tool handling
/// - Malformed args detection and degradation
/// - Tiered description switching
/// - Successful dispatch without degradation

use crate::dispatcher::Dispatcher;
use operon_context_normalize_tools::{ToolCall, ToolCallId};
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
async fn test_malformed_args_marks_tool_degraded() {
    let mut d = Dispatcher::new();
    d.register_fs_tools();

    assert!(!d.is_degraded("read"));

    // Send malformed args — missing required "paths" field
    let result = d.dispatch(make_call("read", json!({ "wrong_key": [] }))).await;

    assert!(result.is_error);
    assert!(d.is_degraded("read"), "read should be degraded after malformed call");
}

#[tokio::test]
async fn test_degraded_tool_uses_detailed_definition() {
    let mut d = Dispatcher::new();
    d.register_fs_tools();

    // Short description is used initially
    let short_desc = d
        .definitions()
        .find(|def| def.name == "read")
        .unwrap()
        .description
        .clone();

    // Trigger degradation
    d.dispatch(make_call("read", json!({ "bad": "args" }))).await;

    // Detailed description is now used
    let detailed_desc = d
        .definitions()
        .find(|def| def.name == "read")
        .unwrap()
        .description
        .clone();

    assert_ne!(short_desc, detailed_desc);
    assert!(
        detailed_desc.len() > short_desc.len(),
        "detailed description should be longer than short"
    );
}

#[tokio::test]
async fn test_other_tools_unaffected_by_degradation() {
    // Once more tools are registered this test verifies that degrading one
    // tool does not affect the description tier of others.
    // For now it verifies the degraded set is tool-specific (not global).
    let mut d = Dispatcher::new();
    d.register_fs_tools();

    // Degrade "read"
    d.dispatch(make_call("read", json!({ "bad": "args" }))).await;

    assert!(d.is_degraded("read"));
    // "write" is not yet implemented but the degraded set must not contain it
    assert!(!d.is_degraded("write"));
    assert!(!d.is_degraded("grep"));
}

#[tokio::test]
async fn test_successful_dispatch_does_not_degrade() {
    use tempfile::TempDir;
    use std::fs;

    let dir = TempDir::new().unwrap();
    let file = dir.path().join("hello.txt");
    fs::write(&file, "hello world\n").unwrap();

    let mut d = Dispatcher::new();
    d.register_fs_tools();

    let result = d
        .dispatch(make_call(
            "read",
            json!({ "paths": [file.to_str().unwrap()] }),
        ))
        .await;

    assert!(!result.is_error);
    assert!(!d.is_degraded("read"));
}
