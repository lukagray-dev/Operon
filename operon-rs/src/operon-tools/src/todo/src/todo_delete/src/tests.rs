//! Comprehensive tests for the todo_delete tool.
//!
//! Tests cover basic deletion, error cases (nonexistent id), remaining count,
//! and persistence verification.

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
// SUCCESS TESTS
// ============================================================================

#[tokio::test]
async fn test_delete_basic() {
    // Test: Create item, delete by id
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
    assert_eq!(output.id, item.id, "deleted id should match");
    assert_eq!(output.remaining, 0, "remaining should be 0");
}

#[tokio::test]
async fn test_delete_returns_correct_id() {
    // Test: Deleted id is returned correctly
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let _item2 = store.create("Task 2".to_string(), None);

    let result = execute(
        call_id("test_delete_returns_correct_id"),
        json!({"id": item1.id}),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_delete_output(&result);
    assert_eq!(output.id, item1.id, "should return the deleted id");
}

#[tokio::test]
async fn test_delete_remaining_count_decrements() {
    // Test: Remaining count decrements correctly
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let _item2 = store.create("Task 2".to_string(), None);
    let _item3 = store.create("Task 3".to_string(), None);

    let result = execute(
        call_id("test_delete_remaining_count_1"),
        json!({"id": item1.id}),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_delete_output(&result);
    assert_eq!(
        output.remaining, 2,
        "remaining should be 2 after deleting 1 of 3"
    );
}

#[tokio::test]
async fn test_delete_multiple_items() {
    // Test: Delete multiple items sequentially
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);
    let item3 = store.create("Task 3".to_string(), None);

    let result1 = execute(
        call_id("test_delete_multiple_1"),
        json!({"id": item1.id}),
        &mut store,
    )
    .await
    .unwrap();
    let output1 = get_delete_output(&result1);
    assert_eq!(output1.remaining, 2);

    let result2 = execute(
        call_id("test_delete_multiple_2"),
        json!({"id": item2.id}),
        &mut store,
    )
    .await
    .unwrap();
    let output2 = get_delete_output(&result2);
    assert_eq!(output2.remaining, 1);

    let result3 = execute(
        call_id("test_delete_multiple_3"),
        json!({"id": item3.id}),
        &mut store,
    )
    .await
    .unwrap();
    let output3 = get_delete_output(&result3);
    assert_eq!(output3.remaining, 0);
}

#[tokio::test]
async fn test_delete_persists_in_store() {
    // Test: Deletion is persisted in the store
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);

    execute(
        call_id("test_delete_persists_1"),
        json!({"id": item1.id}),
        &mut store,
    )
    .await
    .unwrap();

    // Verify by listing
    let items = store.list();
    assert_eq!(items.len(), 1, "should have 1 item remaining");
    assert_eq!(items[0].id, item2.id, "remaining item should be item2");
}

#[tokio::test]
async fn test_delete_specific_item_leaves_others() {
    // Test: Deleting one item leaves others intact
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);
    let item3 = store.create("Task 3".to_string(), None);

    execute(
        call_id("test_delete_specific_item_1"),
        json!({"id": item2.id}),
        &mut store,
    )
    .await
    .unwrap();

    let items = store.list();
    assert_eq!(items.len(), 2, "should have 2 items remaining");
    assert!(
        items.iter().any(|i| i.id == item1.id),
        "item1 should still exist"
    );
    assert!(
        items.iter().any(|i| i.id == item3.id),
        "item3 should still exist"
    );
    assert!(
        !items.iter().any(|i| i.id == item2.id),
        "item2 should be deleted"
    );
}

// ============================================================================
// FAILURE TESTS
// ============================================================================

#[tokio::test]
async fn test_delete_nonexistent_id() {
    // Test: Delete with nonexistent id
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
async fn test_delete_nonexistent_id_does_not_modify_store() {
    // Test: Failed delete doesn't modify the store
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    execute(
        call_id("test_delete_nonexistent_id_does_not_modify_1"),
        json!({"id": "99999"}),
        &mut store,
    )
    .await
    .unwrap();

    // Verify store is unchanged
    let items = store.list();
    assert_eq!(items.len(), 1, "store should be unchanged");
    assert_eq!(items[0].id, item.id, "original item should still exist");
}

#[tokio::test]
async fn test_delete_malformed_json_error() {
    // Test: Malformed JSON (missing required field)
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_delete_malformed_json_error"),
        json!({}),
        &mut store,
    )
    .await;

    assert!(
        result.is_err(),
        "missing id should return Err(TodoDeleteToolError::ArgsParse)"
    );
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[tokio::test]
async fn test_delete_from_empty_store() {
    // Test: Delete from empty store
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
async fn test_delete_same_id_twice() {
    // Test: Attempt to delete the same id twice
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    // First delete should succeed
    let result1 = execute(
        call_id("test_delete_same_id_twice_1"),
        json!({"id": item.id}),
        &mut store,
    )
    .await
    .unwrap();
    assert!(!result1.is_error, "first delete should succeed");

    // Second delete should fail
    let result2 = execute(
        call_id("test_delete_same_id_twice_2"),
        json!({"id": item.id}),
        &mut store,
    )
    .await
    .unwrap();
    assert!(result2.is_error, "second delete should fail");
    assert!(
        get_error_text(&result2).contains("not found"),
        "error message should mention not found"
    );
}
