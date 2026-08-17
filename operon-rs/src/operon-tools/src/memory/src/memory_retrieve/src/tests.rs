//! Tests for the memory_retrieve tool.

use crate::{execute, execute_with_progress};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

async fn fresh_store() -> (MemoryStore, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let store = MemoryStore::connect(tmp.path()).await.unwrap();
    (store, tmp)
}

// ── Single-record mode ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_retrieve_by_id_succeeds() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Find me".to_string(), vec!["x".to_string()]).await.unwrap();

    let result = execute(
        ToolCallId("r1".to_string()),
        json!({"id": mem.id}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => {
            let memories = v["memories"].as_array().unwrap();
            assert_eq!(memories.len(), 1);
            assert_eq!(memories[0]["content"], "Find me");
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_retrieve_by_numeric_id() {
    let (store, _tmp) = fresh_store().await;
    let mem = store.add("Numeric id test".to_string(), vec![]).await.unwrap();
    let numeric_id: i64 = mem.id.parse().unwrap();

    let result = execute(
        ToolCallId("r2".to_string()),
        json!({"id": numeric_id}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
}

#[tokio::test]
async fn test_retrieve_not_found_returns_error() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("r3".to_string()),
        json!({"id": "99999"}),
        &store,
    ).await.unwrap();

    assert!(result.is_error);
    match result.content {
        ToolContent::Text(msg) => assert!(msg.contains("memory not found")),
        _ => panic!("expected text error"),
    }
}

// ── List mode ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_mode_no_args_returns_all() {
    let (store, _tmp) = fresh_store().await;
    for i in 0..5 {
        store.add(format!("Memory {}", i), vec![]).await.unwrap();
    }

    let result = execute(
        ToolCallId("r4".to_string()),
        json!({}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["total"], 5);
            assert_eq!(v["memories"].as_array().unwrap().len(), 5);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_list_mode_with_limit() {
    let (store, _tmp) = fresh_store().await;
    for i in 0..8 {
        store.add(format!("Memory {}", i), vec![]).await.unwrap();
    }

    let result = execute(
        ToolCallId("r5".to_string()),
        json!({"limit": 3}),
        &store,
    ).await.unwrap();

    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["memories"].as_array().unwrap().len(), 3);
            assert_eq!(v["total"], 8, "total reflects the full count, not just this page");
            assert_eq!(v["limit"], 3);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_list_mode_with_offset() {
    let (store, _tmp) = fresh_store().await;
    for i in 0..5 {
        store.add(format!("Memory {}", i), vec![]).await.unwrap();
    }

    let result = execute(
        ToolCallId("r6".to_string()),
        json!({"limit": 10, "offset": 4}),
        &store,
    ).await.unwrap();

    match result.content {
        ToolContent::Json(v) => {
            // offset=4 with 5 total → only 1 remaining
            assert_eq!(v["memories"].as_array().unwrap().len(), 1);
            assert_eq!(v["offset"], 4);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_list_empty_store() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("r7".to_string()),
        json!({}),
        &store,
    ).await.unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["memories"].as_array().unwrap().len(), 0);
            assert_eq!(v["total"], 0);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_list_mode_most_recent_first() {
    let (store, _tmp) = fresh_store().await;
    let a = store.add("First added".to_string(), vec![]).await.unwrap();
    let c = store.add("Last added".to_string(), vec![]).await.unwrap();

    let result = execute(
        ToolCallId("r8".to_string()),
        json!({}),
        &store,
    ).await.unwrap();

    match result.content {
        ToolContent::Json(v) => {
            let memories = v["memories"].as_array().unwrap();
            // Most recent first
            assert_eq!(memories[0]["id"], c.id);
            assert_eq!(memories[1]["id"], a.id);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_output_echoes_limit_and_offset() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("r9".to_string()),
        json!({"limit": 7, "offset": 3}),
        &store,
    ).await.unwrap();

    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["limit"], 7);
            assert_eq!(v["offset"], 3);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_execute_with_progress_works() {
    let (store, _tmp) = fresh_store().await;

    let result = execute_with_progress(
        ToolCallId("r10".to_string()),
        json!({}),
        &store,
        None,
    ).await.unwrap();

    assert!(!result.is_error);
}
