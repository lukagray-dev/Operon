//! Comprehensive tests for the todo_update tool.

use crate::execute;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::{TodoPriority, TodoStore};
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
async fn test_update_status_to_in_progress() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_status_to_in_progress"),
        json!({
            "id": &item.id,
            "status": "in_progress"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error, "expected success");
    let text = extract_text(&result);
    assert!(text.contains("in_progress"));
    assert!(text.contains("Task"));
}

#[tokio::test]
async fn test_update_todo() {
    let mut store = TodoStore::new();
    let item = store.create("Old content".to_string(), None);

    let result = execute(
        call_id("test_update_todo"),
        json!({
            "id": &item.id,
            "todo": "New content"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("New content"));
}

#[tokio::test]
async fn test_update_priority() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), Some(TodoPriority::Low));

    let result = execute(
        call_id("test_update_priority"),
        json!({
            "id": &item.id,
            "priority": "high"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("high"));
}

#[tokio::test]
async fn test_update_nonexistent_id() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_update_nonexistent_id"),
        json!({
            "id": "99999",
            "status": "completed"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(result.is_error, "nonexistent id should be an error");
    let text = extract_text(&result);
    assert!(text.contains("not found"));
}

#[tokio::test]
async fn test_update_no_fields_error() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_no_fields_error"),
        json!({"id": &item.id}),
        &mut store,
    )
    .await
    .unwrap();

    assert!(result.is_error, "no fields to update should be an error");
    let text = extract_text(&result);
    assert!(text.contains("no fields"));
}
