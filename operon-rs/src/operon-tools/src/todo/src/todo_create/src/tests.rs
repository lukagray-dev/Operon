//! Comprehensive tests for the todo_create tool.

use crate::{execute, TodoCreateOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use operon_tools_core::TodoStore;
use serde_json::json;

/// Helper to extract error text from a ToolResult.
fn get_error_text(result: &operon_context_normalize_tools::ToolResult) -> String {
    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content, got {:?}", other),
    }
}

/// Helper to extract TodoCreateOutput from a ToolResult.
fn get_create_output(result: &operon_context_normalize_tools::ToolResult) -> TodoCreateOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize TodoCreateOutput")
        }
        other => panic!("expected Json content, got {:?}", other),
    }
}

fn call_id(n: &str) -> ToolCallId {
    ToolCallId(n.to_string())
}

// ============================================================================
// SUCCESS TESTS (Single & Batch)
// ============================================================================

#[tokio::test]
async fn test_create_basic() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_basic"),
        json!({
            "content": "Fix bug"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error, "expected success");
    let output = get_create_output(&result);
    assert_eq!(output.items.len(), 1);
    assert_eq!(output.items[0].content, "Fix bug");
    assert_eq!(
        output.items[0].status,
        operon_tools_core::TodoStatus::Pending,
        "new items should start as pending"
    );
    assert_eq!(
        output.items[0].priority,
        operon_tools_core::TodoPriority::Medium,
        "default priority should be medium"
    );
    assert_eq!(output.item.as_ref().unwrap().content, "Fix bug");
    assert_eq!(output.total, 1, "total should be 1 after first creation");
}

#[tokio::test]
async fn test_create_with_priority_high() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_with_priority_high"),
        json!({
            "content": "Urgent task",
            "priority": "high"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_create_output(&result);
    assert_eq!(
        output.items[0].priority,
        operon_tools_core::TodoPriority::High,
        "priority should be high"
    );
}

#[tokio::test]
async fn test_create_with_priority_low() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_with_priority_low"),
        json!({
            "content": "Deferred task",
            "priority": "low"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_create_output(&result);
    assert_eq!(
        output.items[0].priority,
        operon_tools_core::TodoPriority::Low,
        "priority should be low"
    );
}

#[tokio::test]
async fn test_create_batch_objects() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_batch_objects"),
        json!({
            "todos": [
                { "content": "Task 1", "priority": "high" },
                { "content": "Task 2", "priority": "medium" },
                { "content": "Task 3", "priority": "low" }
            ]
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_create_output(&result);
    assert_eq!(output.items.len(), 3);
    assert_eq!(output.items[0].id, "1");
    assert_eq!(output.items[0].content, "Task 1");
    assert_eq!(
        output.items[0].priority,
        operon_tools_core::TodoPriority::High
    );
    assert_eq!(output.items[1].id, "2");
    assert_eq!(output.items[2].id, "3");
    assert_eq!(output.total, 3);
}

#[tokio::test]
async fn test_create_batch_strings() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_batch_strings"),
        json!({
            "todos": ["Task Alpha", "Task Beta"]
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_create_output(&result);
    assert_eq!(output.items.len(), 2);
    assert_eq!(output.items[0].content, "Task Alpha");
    assert_eq!(output.items[1].content, "Task Beta");
    assert_eq!(output.total, 2);
}

#[tokio::test]
async fn test_create_root_array() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_root_array"),
        json!([
            { "content": "Root Task 1", "priority": "high" },
            { "content": "Root Task 2" }
        ]),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_create_output(&result);
    assert_eq!(output.items.len(), 2);
    assert_eq!(output.items[0].content, "Root Task 1");
    assert_eq!(output.items[1].content, "Root Task 2");
}

#[tokio::test]
async fn test_create_whitespace_trimmed() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_whitespace_trimmed"),
        json!({
            "content": "  Task with spaces  "
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_create_output(&result);
    assert_eq!(
        output.items[0].content, "Task with spaces",
        "whitespace should be trimmed"
    );
}

// ============================================================================
// FAILURE TESTS
// ============================================================================

#[tokio::test]
async fn test_create_empty_content_error() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_empty_content_error"),
        json!({
            "content": ""
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(result.is_error, "empty content should be an error");
    assert!(
        get_error_text(&result).contains("empty"),
        "error message should mention empty"
    );
}

#[tokio::test]
async fn test_create_empty_batch_item_error() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_empty_batch_item_error"),
        json!({
            "todos": [
                { "content": "Valid task" },
                { "content": "   " }
            ]
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(result.is_error, "empty content in batch should be an error");
    assert!(get_error_text(&result).contains("empty"));
}

#[tokio::test]
async fn test_create_defensive_aliases() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_alias"),
        json!({
            "title": "Build defensive parser",
            "importance": "high"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_create_output(&result);
    assert_eq!(output.items[0].content, "Build defensive parser");
    assert_eq!(
        output.items[0].priority,
        operon_tools_core::TodoPriority::High
    );
}
