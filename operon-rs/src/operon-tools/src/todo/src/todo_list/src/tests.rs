//! Comprehensive tests for the todo_list tool.
//!
//! Tests cover listing all items, filtering by status and priority, status counts,
//! and empty list handling.

use crate::{execute, TodoListOutput};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use operon_tools_core::{TodoPriority, TodoStatus, TodoStore};
use serde_json::json;

/// Helper to extract TodoListOutput from a ToolResult.
fn get_list_output(result: &operon_context_normalize_tools::ToolResult) -> TodoListOutput {
    match &result.content {
        ToolContent::Json(v) => {
            serde_json::from_value(v.clone()).expect("failed to deserialize TodoListOutput")
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
async fn test_list_all() {
    // Test: Create three items, list all
    let mut store = TodoStore::new();
    store.create("Task 1".to_string(), None);
    store.create("Task 2".to_string(), None);
    store.create("Task 3".to_string(), None);

    let result = execute(call_id("test_list_all"), json!({}), &store)
        .await
        .unwrap();

    assert!(!result.is_error, "expected success");
    let output = get_list_output(&result);
    assert_eq!(output.total, 3, "total should be 3");
    assert_eq!(output.items.len(), 3, "items should contain 3 items");
}

#[tokio::test]
async fn test_list_empty() {
    // Test: Fresh store, list
    let store = TodoStore::new();
    let result = execute(call_id("test_list_empty"), json!({}), &store)
        .await
        .unwrap();

    assert!(!result.is_error, "empty list should not be an error");
    let output = get_list_output(&result);
    assert_eq!(output.items.len(), 0, "items should be empty");
    assert_eq!(output.total, 0, "total should be 0");
    assert_eq!(output.pending, 0, "pending count should be 0");
    assert_eq!(output.in_progress, 0, "in_progress count should be 0");
    assert_eq!(output.completed, 0, "completed count should be 0");
}

#[tokio::test]
async fn test_list_filter_by_status_pending() {
    // Test: Create items with different statuses, filter by pending
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);
    let item3 = store.create("Task 3".to_string(), None);

    // Mark item2 as in_progress
    store.update(&item2.id, None, Some(TodoStatus::InProgress), None);
    // Mark item3 as completed
    store.update(&item3.id, None, Some(TodoStatus::Completed), None);

    let result = execute(
        call_id("test_list_filter_by_status_pending"),
        json!({"status": "pending"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.items.len(), 1, "should have 1 pending item");
    assert_eq!(output.items[0].id, item1.id, "pending item should be item1");
    assert_eq!(output.total, 3, "total should still be 3");
}

#[tokio::test]
async fn test_list_filter_by_status_in_progress() {
    // Test: Filter by in_progress status
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let _item2 = store.create("Task 2".to_string(), None);

    store.update(&item1.id, None, Some(TodoStatus::InProgress), None);

    let result = execute(
        call_id("test_list_filter_by_status_in_progress"),
        json!({"status": "in_progress"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.items.len(), 1, "should have 1 in_progress item");
    assert_eq!(output.items[0].id, item1.id);
}

#[tokio::test]
async fn test_list_filter_by_status_completed() {
    // Test: Filter by completed status
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let _item2 = store.create("Task 2".to_string(), None);

    store.update(&item1.id, None, Some(TodoStatus::Completed), None);

    let result = execute(
        call_id("test_list_filter_by_status_completed"),
        json!({"status": "completed"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.items.len(), 1, "should have 1 completed item");
    assert_eq!(output.items[0].id, item1.id);
}

#[tokio::test]
async fn test_list_filter_by_priority_high() {
    // Test: Filter by high priority
    let mut store = TodoStore::new();
    store.create("Task 1".to_string(), Some(TodoPriority::High));
    store.create("Task 2".to_string(), Some(TodoPriority::Medium));
    store.create("Task 3".to_string(), Some(TodoPriority::High));

    let result = execute(
        call_id("test_list_filter_by_priority_high"),
        json!({"priority": "high"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.items.len(), 2, "should have 2 high priority items");
    assert!(output
        .items
        .iter()
        .all(|i| i.priority == TodoPriority::High));
}

#[tokio::test]
async fn test_list_filter_by_priority_low() {
    // Test: Filter by low priority
    let mut store = TodoStore::new();
    store.create("Task 1".to_string(), Some(TodoPriority::Low));
    store.create("Task 2".to_string(), Some(TodoPriority::Medium));

    let result = execute(
        call_id("test_list_filter_by_priority_low"),
        json!({"priority": "low"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.items.len(), 1, "should have 1 low priority item");
    assert_eq!(output.items[0].priority, TodoPriority::Low);
}

#[tokio::test]
async fn test_list_combined_filters() {
    // Test: Filter by both status and priority
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), Some(TodoPriority::High));
    let item2 = store.create("Task 2".to_string(), Some(TodoPriority::High));
    let _item3 = store.create("Task 3".to_string(), Some(TodoPriority::Low));

    store.update(&item1.id, None, Some(TodoStatus::InProgress), None);
    store.update(&item2.id, None, Some(TodoStatus::Completed), None);

    let result = execute(
        call_id("test_list_combined_filters"),
        json!({"status": "in_progress", "priority": "high"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(
        output.items.len(),
        1,
        "should have 1 item matching both filters"
    );
    assert_eq!(output.items[0].id, item1.id);
}

#[tokio::test]
async fn test_list_status_counts_all_pending() {
    // Test: All items pending, verify counts
    let mut store = TodoStore::new();
    store.create("Task 1".to_string(), None);
    store.create("Task 2".to_string(), None);
    store.create("Task 3".to_string(), None);

    let result = execute(
        call_id("test_list_status_counts_all_pending"),
        json!({}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.pending, 3, "pending count should be 3");
    assert_eq!(output.in_progress, 0, "in_progress count should be 0");
    assert_eq!(output.completed, 0, "completed count should be 0");
}

#[tokio::test]
async fn test_list_status_counts_mixed() {
    // Test: Mixed statuses, verify counts
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);
    let item3 = store.create("Task 3".to_string(), None);
    let _item4 = store.create("Task 4".to_string(), None);

    store.update(&item1.id, None, Some(TodoStatus::InProgress), None);
    store.update(&item2.id, None, Some(TodoStatus::InProgress), None);
    store.update(&item3.id, None, Some(TodoStatus::Completed), None);

    let result = execute(call_id("test_list_status_counts_mixed"), json!({}), &store)
        .await
        .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.pending, 1, "pending count should be 1");
    assert_eq!(output.in_progress, 2, "in_progress count should be 2");
    assert_eq!(output.completed, 1, "completed count should be 1");
    assert_eq!(output.total, 4, "total should be 4");
}

#[tokio::test]
async fn test_list_counts_unaffected_by_filter() {
    // Test: Counts are always from full unfiltered list, even when filtering
    let mut store = TodoStore::new();
    let item1 = store.create("Task 1".to_string(), None);
    let item2 = store.create("Task 2".to_string(), None);
    let _item3 = store.create("Task 3".to_string(), None);

    store.update(&item1.id, None, Some(TodoStatus::Completed), None);
    store.update(&item2.id, None, Some(TodoStatus::Completed), None);

    // Filter to show only pending items
    let result = execute(
        call_id("test_list_counts_unaffected_by_filter"),
        json!({"status": "pending"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.items.len(), 1, "filtered items should be 1");
    assert_eq!(
        output.pending, 1,
        "pending count should be 1 (from full list)"
    );
    assert_eq!(
        output.completed, 2,
        "completed count should be 2 (from full list)"
    );
    assert_eq!(output.total, 3, "total should be 3 (from full list)");
}

// ============================================================================
// MALFORMED ARGS TESTS
// ============================================================================

#[tokio::test]
async fn test_list_invalid_status_error() {
    // Test: Invalid status value
    let store = TodoStore::new();
    let result = execute(
        call_id("test_list_invalid_status_error"),
        json!({"status": "invalid_status"}),
        &store,
    )
    .await;

    assert!(
        result.is_err(),
        "invalid status should return Err(TodoListToolError::ArgsParse)"
    );
}

#[tokio::test]
async fn test_list_invalid_priority_error() {
    // Test: Invalid priority value
    let store = TodoStore::new();
    let result = execute(
        call_id("test_list_invalid_priority_error"),
        json!({"priority": "invalid_priority"}),
        &store,
    )
    .await;

    assert!(
        result.is_err(),
        "invalid priority should return Err(TodoListToolError::ArgsParse)"
    );
}

#[tokio::test]
async fn test_list_defensive_aliases() {
    let mut store = TodoStore::new();
    store.create("Task 1".to_string(), Some(TodoPriority::High));

    let result = execute(
        call_id("test_alias"),
        json!({
            "importance": "high"
        }),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_list_output(&result);
    assert_eq!(output.items.len(), 1);
}
