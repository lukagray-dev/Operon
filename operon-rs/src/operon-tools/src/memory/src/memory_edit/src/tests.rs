//! Tests for the memory_edit tool.

use crate::{execute, execute_with_progress};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

async fn fresh_store() -> (MemoryStore, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let store = MemoryStore::connect(tmp.path()).await.unwrap();
    (store, tmp)
}

#[tokio::test]
async fn test_edit_content_succeeds() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Original".to_string(), vec![]).await.unwrap();

    let result = execute(
        ToolCallId("c1".to_string()),
        json!({"id": mem.id, "content": "Updated"}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => assert_eq!(v["memory"]["content"], "Updated"),
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_edit_tags_only_succeeds() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Content".to_string(), vec!["old".to_string()]).await.unwrap();

    let result = execute(
        ToolCallId("c2".to_string()),
        json!({"id": mem.id, "tags": ["new1", "new2"]}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => assert_eq!(v["memory"]["tags"], json!(["new1", "new2"])),
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_edit_numeric_id() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Test".to_string(), vec![]).await.unwrap();
    let numeric_id: i64 = mem.id.parse().unwrap();

    let result = execute(
        ToolCallId("c3".to_string()),
        json!({"id": numeric_id, "content": "Numeric id works"}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
}

#[tokio::test]
async fn test_edit_no_fields_returns_error() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Test".to_string(), vec![]).await.unwrap();

    let result = execute(
        ToolCallId("c4".to_string()),
        json!({"id": mem.id}),
        &store,
    ).await.unwrap();

    assert!(result.is_error);
    match result.content {
        ToolContent::Text(msg) => assert!(msg.contains("no fields to update")),
        _ => panic!("expected text error"),
    }
}

#[tokio::test]
async fn test_edit_empty_content_returns_error() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Test".to_string(), vec![]).await.unwrap();

    let result = execute(
        ToolCallId("c5".to_string()),
        json!({"id": mem.id, "content": "   "}),
        &store,
    ).await.unwrap();

    assert!(result.is_error);
    match result.content {
        ToolContent::Text(msg) => assert!(msg.contains("content is empty")),
        _ => panic!("expected text error"),
    }
}

#[tokio::test]
async fn test_edit_not_found_returns_error() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("c6".to_string()),
        json!({"id": "99999", "content": "New content"}),
        &store,
    ).await.unwrap();

    assert!(result.is_error);
    match result.content {
        ToolContent::Text(msg) => assert!(msg.contains("memory not found")),
        _ => panic!("expected text error"),
    }
}

#[tokio::test]
async fn test_edit_missing_id_returns_parse_error() {
    let (store, _tmp) = fresh_store().await;
    let err = execute(
        ToolCallId("c7".to_string()),
        json!({"content": "No id"}),
        &store,
    ).await;
    assert!(err.is_err(), "missing id should return ArgsParse error");
}

#[tokio::test]
async fn test_edit_created_at_unchanged() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Original".to_string(), vec![]).await.unwrap();
    let original_created = mem.created_at.clone();

    let result = execute(
        ToolCallId("c8".to_string()),
        json!({"id": mem.id, "content": "Updated content"}),
        &store,
    ).await.unwrap();

    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["memory"]["created_at"], original_created,
                "created_at must never change on edit");
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_execute_with_progress_works() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Test".to_string(), vec![]).await.unwrap();

    let result = execute_with_progress(
        ToolCallId("c9".to_string()),
        json!({"id": mem.id, "content": "Progress test"}),
        &store,
        None,
    ).await.unwrap();

    assert!(!result.is_error);
}
