//! Comprehensive tests for the todo_list tool.

use crate::execute;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use operon_tools_core::{TodoStatus, TodoStore};
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
async fn test_list_all() {
    let mut store = TodoStore::new();
    store.create("Task 1".to_string(), None);
    store.create("Task 2".to_string(), None);
    store.create("Task 3".to_string(), None);

    let result = execute(call_id("test_list_all"), json!({}), &store)
        .await
        .unwrap();

    assert!(!result.is_error, "expected success");
    let text = extract_text(&result);
    assert!(text.contains("#1 [pending] [medium] Task 1"));
    assert!(text.contains("#2 [pending] [medium] Task 2"));
    assert!(text.contains("#3 [pending] [medium] Task 3"));
    assert!(text.contains("Total: 3 (3 pending"));
}

#[tokio::test]
async fn test_list_empty() {
    let store = TodoStore::new();
    let result = execute(call_id("test_list_empty"), json!({}), &store)
        .await
        .unwrap();

    assert!(!result.is_error, "empty list should not be an error");
    let text = extract_text(&result);
    assert!(text.contains("No todos yet."));
}

#[tokio::test]
async fn test_list_filter_by_status_pending() {
    let mut store = TodoStore::new();
    let _item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);
    let item3 = store.create("Task 3".to_string(), None);

    store.update(&item2.id, None, Some(TodoStatus::InProgress), None);
    store.update(&item3.id, None, Some(TodoStatus::Completed), None);

    let result = execute(
        call_id("test_list_filter_by_status_pending"),
        json!({"status": "pending"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let text = extract_text(&result);
    assert!(text.contains("#1 [pending] [medium] Task 1"));
    assert!(!text.contains("Task 2"));
    assert!(!text.contains("Task 3"));
    assert!(text.contains("Total: 3 (1 pending"));
}
