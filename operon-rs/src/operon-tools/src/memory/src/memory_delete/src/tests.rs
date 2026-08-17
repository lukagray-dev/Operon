//! Tests for the memory_delete tool.

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
async fn test_delete_existing_succeeds() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("To be deleted".to_string(), vec![]).await.unwrap();

    let result = execute(
        ToolCallId("d1".to_string()),
        json!({"id": mem.id}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["id"], mem.id);
            assert_eq!(v["remaining"], 0);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_delete_remaining_count_decreases() {
    let (store, _tmp) = fresh_store().await;
    let m1 = store.add("First".to_string(), vec![]).await.unwrap();
    store.add("Second".to_string(), vec![]).await.unwrap();

    let result = execute(
        ToolCallId("d2".to_string()),
        json!({"id": m1.id}),
        &store,
    ).await.unwrap();

    match result.content {
        ToolContent::Json(v) => assert_eq!(v["remaining"], 1),
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_delete_not_found_returns_error() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("d3".to_string()),
        json!({"id": "99999"}),
        &store,
    ).await.unwrap();

    assert!(result.is_error);
    match result.content {
        ToolContent::Text(msg) => assert!(msg.contains("memory not found")),
        _ => panic!("expected text error"),
    }
}

#[tokio::test]
async fn test_delete_numeric_id() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Test".to_string(), vec![]).await.unwrap();
    let numeric_id: i64 = mem.id.parse().unwrap();

    let result = execute(
        ToolCallId("d4".to_string()),
        json!({"id": numeric_id}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
}

#[tokio::test]
async fn test_delete_memory_removed_from_get() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Gone".to_string(), vec![]).await.unwrap();

    execute(
        ToolCallId("d5".to_string()),
        json!({"id": mem.id}),
        &store,
    ).await.unwrap();

    let fetched = store.get(&mem.id).await.unwrap();
    assert!(fetched.is_none(), "deleted memory should not be retrievable");
}

#[tokio::test]
async fn test_delete_missing_id_returns_parse_error() {
    let (store, _tmp) = fresh_store().await;
    let err = execute(
        ToolCallId("d6".to_string()),
        json!({}),
        &store,
    ).await;
    assert!(err.is_err(), "missing id should return ArgsParse error");
}

#[tokio::test]
async fn test_delete_with_progress() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Progress delete".to_string(), vec![]).await.unwrap();

    let result = execute_with_progress(
        ToolCallId("d7".to_string()),
        json!({"id": mem.id}),
        &store,
        None,
    ).await.unwrap();

    assert!(!result.is_error);
}
