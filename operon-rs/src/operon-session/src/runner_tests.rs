use super::*;

use operon_context_normalize_tools::{ToolCall, ToolCallId, ToolContent, ToolResult};
use serde_json::json;

// Build a tiny ToolCall fixture so the helper tests stay focused and readable.
fn make_call(name: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: ToolCallId(format!("call_{name}")),
        name: name.to_string(),
        arguments,
    }
}

#[test]
fn command_matches_only_accepts_cancel_or_the_matching_approval_id() {
    // Cancel is always relevant because it should stop the session immediately.
    assert!(command_matches(&SessionCommand::Cancel, None));
    assert!(command_matches(&SessionCommand::Cancel, Some("anything")));

    // Approve/Deny only count when the approval ID matches the pending request.
    assert!(command_matches(
        &SessionCommand::Approve {
            id: "approval-1".to_string(),
        },
        Some("approval-1"),
    ));
    assert!(command_matches(
        &SessionCommand::Deny {
            id: "approval-1".to_string(),
        },
        Some("approval-1"),
    ));

    assert!(!command_matches(
        &SessionCommand::Approve {
            id: "approval-1".to_string(),
        },
        Some("approval-2"),
    ));
    assert!(!command_matches(
        &SessionCommand::Deny {
            id: "approval-1".to_string(),
        },
        None,
    ));
}

#[test]
fn policy_path_for_call_extracts_the_correct_anchor() {
    // read uses the first entry from the paths array.
    let read_call = make_call("read", json!({ "paths": ["/tmp/a.txt", "/tmp/b.txt"] }));
    assert_eq!(
        policy_path_for_call(&read_call).as_deref(),
        Some("/tmp/a.txt")
    );

    // bash uses cwd as its policy anchor.
    let bash_call = make_call("bash", json!({ "command": "ls", "cwd": "/tmp/work" }));
    assert_eq!(
        policy_path_for_call(&bash_call).as_deref(),
        Some("/tmp/work")
    );

    // Other filesystem tools use the path field.
    let write_call = make_call("write", json!({ "path": "/tmp/w.txt" }));
    assert_eq!(
        policy_path_for_call(&write_call).as_deref(),
        Some("/tmp/w.txt")
    );

    // Global tools should not have a path anchor.
    let web_call = make_call("web_search", json!({ "query": "hello" }));
    assert_eq!(policy_path_for_call(&web_call), None);
}

#[test]
fn context_usage_event_reports_window_and_utilization() {
    let budget = TokenBudget::new(200_000, 0.90).expect("valid token budget");
    let event = context_usage_event(&budget, 150_000);

    match event {
        SessionEvent::ContextUsageUpdated {
            current_context_tokens,
            context_window,
            remaining_context_tokens,
            utilization,
            compaction_limit,
        } => {
            assert_eq!(current_context_tokens, 150_000);
            assert_eq!(context_window, 200_000);
            assert_eq!(remaining_context_tokens, 50_000);
            assert_eq!(compaction_limit, 180_000);
            assert!((utilization - 0.75).abs() < 1e-6);
        }
        other => panic!("unexpected context usage event: {:?}", other),
    }
}

#[test]
fn opaque_permission_denied_result_is_generic_and_safe_for_the_model() {
    // Policy denials should never leak internal policy details to the model.
    let call = make_call("write", json!({ "path": "/tmp/secret.txt" }));
    let result = opaque_permission_denied_result(&call);

    assert_eq!(result.call_id, call.id);
    assert_eq!(result.name, "write");
    assert!(result.is_error);

    match result.content {
        ToolContent::Text(message) => {
            assert_eq!(message, "Tool not available.");
        }
        other => panic!("unexpected tool content for denied call: {:?}", other),
    }
}

#[test]
fn tool_result_content_json_serializes_text_and_json_cleanly() {
    // Text content should be passed through unchanged.
    let text_result = ToolResult {
        call_id: ToolCallId("call_text".to_string()),
        name: "write".to_string(),
        content: ToolContent::Text("plain text".to_string()),
        is_error: false,
    };
    assert_eq!(tool_result_content_json(&text_result), "plain text");

    // JSON content should be rendered as a compact JSON string.
    let json_result = ToolResult {
        call_id: ToolCallId("call_json".to_string()),
        name: "read".to_string(),
        content: ToolContent::Json(json!({ "ok": true })),
        is_error: false,
    };
    assert_eq!(tool_result_content_json(&json_result), "{\"ok\":true}");
}
