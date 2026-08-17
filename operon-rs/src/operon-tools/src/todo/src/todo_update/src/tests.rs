//! Comprehensive tests for the todo_update tool.

use crate::{execute, TodoUpdateOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use operon_tools_core::{TodoPriority, TodoStatus, TodoStore};
use serde_json::json;

/// Helper to extract error text from a ToolResult.
fn get_error_text(result: &operon_context_normalize_tools::ToolResult) -> String {
    match &result.content {
        ToolContent::Text(t) => t.clone(),
        other => panic!("expected Text content, got {:?}", other),
    }
}

/// Helper to extract TodoUpdateOutput from a ToolResult.
fn get_update_output(result: &operon_context_normalize_tools::ToolResult) -> TodoUpdateOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize TodoUpdateOutput")
        }
        other => panic!("expected Json content, got {:?}", other),
    }
}

fn call_id(n: &str) -> ToolCallId {
    ToolCallId(n.to_string())
}

// ============================================================================
// SUCCESS TESTS (Single, Batch & Bulk)
// ============================================================================

#[tokio::test]
async fn test_update_status_to_in_progress() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_status_to_in_progress"),
        json!({
            "id": item.id,
            "status": "in_progress"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error, "expected success");
    let output = get_update_output(&result);
    assert_eq!(output.items.len(), 1);
    assert_eq!(output.items[0].status, TodoStatus::InProgress);
    assert_eq!(output.items[0].content, "Task");
    assert_eq!(output.item.unwrap().status, TodoStatus::InProgress);
}

#[tokio::test]
async fn test_update_status_to_completed() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_status_to_completed"),
        json!({
            "id": item.id,
            "status": "completed"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(output.items[0].status, TodoStatus::Completed);
}

#[tokio::test]
async fn test_update_batch_distinct_items() {
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);

    let result = execute(
        call_id("test_update_batch"),
        json!({
            "todos": [
                { "id": item1.id, "status": "completed" },
                { "id": item2.id, "status": "in_progress", "priority": "high" }
            ]
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(output.items.len(), 2);
    assert_eq!(output.items[0].status, TodoStatus::Completed);
    assert_eq!(output.items[1].status, TodoStatus::InProgress);
    assert_eq!(output.items[1].priority, TodoPriority::High);
}

#[tokio::test]
async fn test_update_bulk_ids() {
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);

    let result = execute(
        call_id("test_bulk_ids"),
        json!({
            "ids": [item1.id, item2.id],
            "status": "completed"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(output.items.len(), 2);
    assert_eq!(output.items[0].status, TodoStatus::Completed);
    assert_eq!(output.items[1].status, TodoStatus::Completed);
}

#[tokio::test]
async fn test_update_root_array() {
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);

    let result = execute(
        call_id("test_root_array"),
        json!([
            { "id": item1.id, "status": "completed" },
            { "id": item2.id, "content": "Updated Task 2" }
        ]),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(output.items.len(), 2);
    assert_eq!(output.items[0].status, TodoStatus::Completed);
    assert_eq!(output.items[1].content, "Updated Task 2");
}

#[tokio::test]
async fn test_update_content_with_whitespace_trimmed() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_content_with_whitespace_trimmed"),
        json!({
            "id": item.id,
            "content": "  New content  "
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(
        output.items[0].content, "New content",
        "whitespace should be trimmed"
    );
}

// ============================================================================
// FAILURE TESTS
// ============================================================================

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
    assert!(
        get_error_text(&result).contains("not found"),
        "error message should mention not found"
    );
}

#[tokio::test]
async fn test_update_no_fields_error() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_no_fields_error"),
        json!({"id": item.id}),
        &mut store,
    )
    .await
    .unwrap();

    assert!(result.is_error, "no fields to update should be an error");
    assert!(
        get_error_text(&result).contains("no fields"),
        "error message should mention no fields"
    );
}

#[tokio::test]
async fn test_update_empty_content_error() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_empty_content_error"),
        json!({
            "id": item.id,
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
async fn test_update_defensive_aliases_and_numeric_id() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);
    let numeric_id: u64 = item.id.parse().unwrap();

    let result = execute(
        call_id("test_alias"),
        json!({
            "todo_id": numeric_id,
            "title": "Renamed task",
            "state": "completed"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(output.items[0].content, "Renamed task");
    assert_eq!(output.items[0].status, TodoStatus::Completed);
}
