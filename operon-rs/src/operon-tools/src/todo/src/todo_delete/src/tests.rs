//! Comprehensive tests for the todo_delete tool.

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
async fn test_delete_basic() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);

    let result = execute(
        call_id("test_delete_basic"),
        json!({"id": &item.id}),
        &mut store,
    )
    .await
    .unwrap();

    assert!(!result.is_error, "expected success");
    let text = extract_text(&result);
    assert!(text.contains("Deleted #1. 0 todo(s) remaining."));
}

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
    let text = extract_text(&result);
    assert!(text.contains("not found"));
}
