//! Comprehensive tests for the todo_create tool.
//!
//! Tests cover success cases (basic creation, with priority), failure cases (empty content),
//! and ID uniqueness verification.

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
// SUCCESS TESTS
// ============================================================================

#[tokio::test]
async fn test_create_basic() {
    // Test: Create with content "Fix bug", no priority
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
    assert_eq!(output.item.content, "Fix bug");
    assert_eq!(
        output.item.status,
        operon_tools_core::TodoStatus::Pending,
        "new items should start as pending"
    );
    assert_eq!(
        output.item.priority,
        operon_tools_core::TodoPriority::Medium,
        "default priority should be medium"
    );
    assert!(!output.item.id.is_empty(), "id should be assigned");
    assert_eq!(output.total, 1, "total should be 1 after first creation");
}

#[tokio::test]
async fn test_create_with_priority_high() {
    // Test: Create with priority: "high"
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
        output.item.priority,
        operon_tools_core::TodoPriority::High,
        "priority should be high"
    );
}

#[tokio::test]
async fn test_create_with_priority_low() {
    // Test: Create with priority: "low"
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
        output.item.priority,
        operon_tools_core::TodoPriority::Low,
        "priority should be low"
    );
}

#[tokio::test]
async fn test_create_whitespace_trimmed() {
    // Test: Create with leading/trailing whitespace
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
        output.item.content, "Task with spaces",
        "whitespace should be trimmed"
    );
}

#[tokio::test]
async fn test_create_total_count_increments() {
    // Test: Create multiple items, verify total count increments
    let mut store = TodoStore::new();

    let result1 = execute(
        call_id("test_create_total_1"),
        json!({"content": "Task 1"}),
        &mut store,
    )
    .await
    .unwrap();
    let output1 = get_create_output(&result1);
    assert_eq!(output1.total, 1);

    let result2 = execute(
        call_id("test_create_total_2"),
        json!({"content": "Task 2"}),
        &mut store,
    )
    .await
    .unwrap();
    let output2 = get_create_output(&result2);
    assert_eq!(output2.total, 2);

    let result3 = execute(
        call_id("test_create_total_3"),
        json!({"content": "Task 3"}),
        &mut store,
    )
    .await
    .unwrap();
    let output3 = get_create_output(&result3);
    assert_eq!(output3.total, 3);
}

// ============================================================================
// FAILURE TESTS
// ============================================================================

#[tokio::test]
async fn test_create_empty_content_error() {
    // Test: Pass content: ""
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
async fn test_create_whitespace_only_error() {
    // Test: Pass content with only whitespace
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_whitespace_only_error"),
        json!({
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
async fn test_create_malformed_json_error() {
    // Test: Malformed JSON (missing required field)
    let mut store = TodoStore::new();
    let result = execute(
        call_id("test_create_malformed_json_error"),
        json!({
            "priority": "high"
            // missing "content"
        }),
        &mut store,
    )
    .await;

    assert!(
        result.is_err(),
        "malformed args should return Err(TodoCreateToolError::ArgsParse)"
    );
}

// ============================================================================
// ID UNIQUENESS TESTS
// ============================================================================

#[tokio::test]
async fn test_ids_are_unique() {
    // Test: Create three items, verify all ids are distinct
    let mut store = TodoStore::new();

    let result1 = execute(
        call_id("test_ids_unique_1"),
        json!({"content": "Task 1"}),
        &mut store,
    )
    .await
    .unwrap();
    let output1 = get_create_output(&result1);

    let result2 = execute(
        call_id("test_ids_unique_2"),
        json!({"content": "Task 2"}),
        &mut store,
    )
    .await
    .unwrap();
    let output2 = get_create_output(&result2);

    let result3 = execute(
        call_id("test_ids_unique_3"),
        json!({"content": "Task 3"}),
        &mut store,
    )
    .await
    .unwrap();
    let output3 = get_create_output(&result3);

    assert_ne!(output1.item.id, output2.item.id, "ids should be unique");
    assert_ne!(output2.item.id, output3.item.id, "ids should be unique");
    assert_ne!(output1.item.id, output3.item.id, "ids should be unique");
}

#[tokio::test]
async fn test_ids_increment_sequentially() {
    // Test: Create items sequentially, verify ids are "1", "2", "3"
    let mut store = TodoStore::new();

    let result1 = execute(
        call_id("test_ids_increment_1"),
        json!({"content": "Task 1"}),
        &mut store,
    )
    .await
    .unwrap();
    let output1 = get_create_output(&result1);
    assert_eq!(output1.item.id, "1", "first id should be '1'");

    let result2 = execute(
        call_id("test_ids_increment_2"),
        json!({"content": "Task 2"}),
        &mut store,
    )
    .await
    .unwrap();
    let output2 = get_create_output(&result2);
    assert_eq!(output2.item.id, "2", "second id should be '2'");

    let result3 = execute(
        call_id("test_ids_increment_3"),
        json!({"content": "Task 3"}),
        &mut store,
    )
    .await
    .unwrap();
    let output3 = get_create_output(&result3);
    assert_eq!(output3.item.id, "3", "third id should be '3'");
}
