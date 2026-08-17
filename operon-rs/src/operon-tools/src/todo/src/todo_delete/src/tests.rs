//! Comprehensive tests for the todo_delete tool.

use crate::{execute, TodoDeleteOutput};
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

/// Helper to extract TodoDeleteOutput from a ToolResult.
fn get_delete_output(result: &operon_context_normalize_tools::ToolResult) -> TodoDeleteOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize TodoDeleteOutput")
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
async fn test_delete_basic() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_delete_basic"),
        json!({"id": item.id}),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error, "expected success");
    let output = get_delete_output(&result);
    assert_eq!(output.ids, vec![item.id.clone()]);
    assert_eq!(output.id.unwrap(), item.id);
    assert_eq!(output.remaining, 0);
}

#[tokio::test]
async fn test_delete_batch_ids() {
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);
    let item3 = store.create("Task 3".to_string(), None);

    let result = execute(
        call_id("test_delete_batch"),
        json!({
            "ids": [item1.id.clone(), item3.id.clone()]
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_delete_output(&result);
    assert_eq!(output.ids, vec![item1.id, item3.id]);
    assert_eq!(output.remaining, 1);
    assert_eq!(store.list()[0].id, item2.id);
}

#[tokio::test]
async fn test_delete_root_array() {
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);

    let result = execute(
        call_id("test_delete_root_array"),
        json!([item1.id.clone(), item2.id.clone()]),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_delete_output(&result);
    assert_eq!(output.ids.len(), 2);
    assert_eq!(output.remaining, 0);
}

#[tokio::test]
async fn test_delete_object_array() {
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);

    let result = execute(
        call_id("test_delete_object_array"),
        json!([
            { "id": item1.id.clone() },
            { "id": item2.id.clone() }
        ]),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_delete_output(&result);
    assert_eq!(output.ids.len(), 2);
    assert_eq!(output.remaining, 0);
}

// ============================================================================
// FAILURE TESTS
// ============================================================================

#[tokio::test]
async fn test_delete_nonexistent_id() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_delete_nonexistent_id"),
        json!({"id": "99999"}),
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
async fn test_delete_from_empty_store() {
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_delete_from_empty_store"),
        json!({"id": "1"}),
        &mut store,
    )
    .await
    .unwrap();

    assert!(
        result.is_error,
        "deleting from empty store should be an error"
    );
}

#[tokio::test]
async fn test_delete_defensive_aliases_and_numeric_id() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);
    let numeric_id: u64 = item.id.parse().unwrap();

    let result = execute(
        call_id("test_alias"),
        json!({
            "todo_id": numeric_id
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_delete_output(&result);
    assert_eq!(output.ids, vec![item.id]);
    assert_eq!(output.remaining, 0);
}
