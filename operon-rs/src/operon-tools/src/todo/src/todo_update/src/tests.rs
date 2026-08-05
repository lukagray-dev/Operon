//! Comprehensive tests for the todo_update tool.
//!
//! Tests cover updating status, content, and priority; partial updates;
//! error cases (no fields, nonexistent id, empty content); and validation.

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
// SUCCESS TESTS
// ============================================================================

#[tokio::test]
async fn test_update_status_to_in_progress() {
    // Test: Update status from pending to in_progress
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
    assert_eq!(
        output.item.status,
        TodoStatus::InProgress,
        "status should be in_progress"
    );
    assert_eq!(output.item.content, "Task", "content should be unchanged");
}

#[tokio::test]
async fn test_update_status_to_completed() {
    // Test: Update status to completed
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
    assert_eq!(output.item.status, TodoStatus::Completed);
}

#[tokio::test]
async fn test_update_content() {
    // Test: Update content
    let mut store = TodoStore::new();
    let item = store.create("Old content".to_string(), None);

    let result = execute(
        call_id("test_update_content"),
        json!({
            "id": item.id,
            "content": "New content"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(
        output.item.content, "New content",
        "content should be updated"
    );
    assert_eq!(
        output.item.status,
        TodoStatus::Pending,
        "status should be unchanged"
    );
}

#[tokio::test]
async fn test_update_priority() {
    // Test: Update priority
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), Some(TodoPriority::Low));

    let result = execute(
        call_id("test_update_priority"),
        json!({
            "id": item.id,
            "priority": "high"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(
        output.item.priority,
        TodoPriority::High,
        "priority should be high"
    );
}

#[tokio::test]
async fn test_update_multiple_fields() {
    // Test: Update multiple fields at once
    let mut store = TodoStore::new();
    let item = store.create("Old".to_string(), Some(TodoPriority::Low));

    let result = execute(
        call_id("test_update_multiple_fields"),
        json!({
            "id": item.id,
            "content": "New",
            "status": "in_progress",
            "priority": "high"
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    let output = get_update_output(&result);
    assert_eq!(output.item.content, "New");
    assert_eq!(output.item.status, TodoStatus::InProgress);
    assert_eq!(output.item.priority, TodoPriority::High);
}

#[tokio::test]
async fn test_update_content_with_whitespace_trimmed() {
    // Test: Content whitespace is trimmed
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
        output.item.content, "New content",
        "whitespace should be trimmed"
    );
}

#[tokio::test]
async fn test_update_persists_in_store() {
    // Test: Update is persisted in the store
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    execute(
        call_id("test_update_persists_1"),
        json!({
            "id": item.id,
            "status": "completed"
        }),
        &mut store,
    )
    .await
    .unwrap();

    // Verify by listing
    let items = store.list();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].status, TodoStatus::Completed);
}

// ============================================================================
// FAILURE TESTS
// ============================================================================

#[tokio::test]
async fn test_update_nonexistent_id() {
    // Test: Update with nonexistent id
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
    // Test: Update with only id, no other fields
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
    // Test: Update content to empty string
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
async fn test_update_whitespace_only_content_error() {
    // Test: Update content to whitespace-only
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_whitespace_only_content_error"),
        json!({
            "id": item.id,
            "content": "   \t\n  "
        }),
        &mut store,
    )
    .await
    .unwrap();

    assert!(
        result.is_error,
        "whitespace-only content should be an error"
    );
    assert!(
        get_error_text(&result).contains("empty"),
        "error message should mention empty"
    );
}

#[tokio::test]
async fn test_update_malformed_json_error() {
    // Test: Malformed JSON (invalid status value)
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_update_malformed_json_error"),
        json!({
            "id": item.id,
            "status": "invalid_status"
        }),
        &mut store,
    )
    .await;

    assert!(
        result.is_err(),
        "invalid status should return Err(TodoUpdateToolError::ArgsParse)"
    );
}

// ============================================================================
// PARTIAL UPDATE TESTS
// ============================================================================

#[tokio::test]
async fn test_update_only_status_leaves_others_unchanged() {
    // Test: Updating only status leaves content and priority unchanged
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), Some(TodoPriority::High));

    execute(
        call_id("test_update_only_status_1"),
        json!({
            "id": item.id,
            "status": "in_progress"
        }),
        &mut store,
    )
    .await
    .unwrap();

    let items = store.list();
    assert_eq!(items[0].content, "Task", "content should be unchanged");
    assert_eq!(
        items[0].priority,
        TodoPriority::High,
        "priority should be unchanged"
    );
    assert_eq!(
        items[0].status,
        TodoStatus::InProgress,
        "status should be updated"
    );
}

#[tokio::test]
async fn test_update_only_priority_leaves_others_unchanged() {
    // Test: Updating only priority leaves content and status unchanged
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), Some(TodoPriority::Low));

    execute(
        call_id("test_update_only_priority_1"),
        json!({
            "id": item.id,
            "priority": "high"
        }),
        &mut store,
    )
    .await
    .unwrap();

    let items = store.list();
    assert_eq!(items[0].content, "Task", "content should be unchanged");
    assert_eq!(
        items[0].status,
        TodoStatus::Pending,
        "status should be unchanged"
    );
    assert_eq!(
        items[0].priority,
        TodoPriority::High,
        "priority should be updated"
    );
}

#[tokio::test]
async fn test_update_only_content_leaves_others_unchanged() {
    // Test: Updating only content leaves status and priority unchanged
    let mut store = TodoStore::new();
    let item = store.create("Old".to_string(), Some(TodoPriority::High));
    store.update(&item.id, None, Some(TodoStatus::InProgress), None);

    execute(
        call_id("test_update_only_content_1"),
        json!({
            "id": item.id,
            "content": "New"
        }),
        &mut store,
    )
    .await
    .unwrap();

    let items = store.list();
    assert_eq!(items[0].content, "New", "content should be updated");
    assert_eq!(
        items[0].status,
        TodoStatus::InProgress,
        "status should be unchanged"
    );
    assert_eq!(
        items[0].priority,
        TodoPriority::High,
        "priority should be unchanged"
    );
}
