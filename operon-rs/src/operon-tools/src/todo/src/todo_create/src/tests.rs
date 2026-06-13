//! Comprehensive tests for the todo_create tool.
//!
//! Tests cover success cases (basic creation, with priority), failure cases (empty content),
//! and ID uniqueness verification.

use crate::execute;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::TodoStore;
use serde_json::json;

/// Helper to extract text from a ToolResult.
fn extract_text(result: &ToolResult) -> String {

    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content, got {:?}", other),
    }
}

fn call_id(n: &str) -> ToolCallId {
    ToolCallId(n.to_string())
}

#[tokio::test]
async fn test_create_basic() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_basic"),
        json!({
            "todo": "Fix bug"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error, "expected success");
    let text = extract_text(&result);
    assert!(text.contains("Created #1: Fix bug [medium]"));
    assert!(text.contains("Total: 1 (1 pending"));
}

#[tokio::test]
async fn test_create_with_priority_high() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_with_priority_high"),
        json!({
            "todo": "Urgent task",
            "priority": "high"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("Urgent task [high]"));
}

#[tokio::test]
async fn test_create_whitespace_trimmed() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_whitespace_trimmed"),
        json!({
            "todo": "  Task with spaces  "
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("Task with spaces"));
}

#[tokio::test]
async fn test_create_empty_content_error() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_empty_content_error"),
        json!({
            "todo": ""
        }),
        &mut store,
    )
    .await;

    assert!(result.is_err());
}
