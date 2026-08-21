//! Tests for the memory_search tool.

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
async fn test_search_finds_matching_memory() {
    let (store, _tmp) = fresh_store().await;
    store
        .add("User loves Rust programming".to_string(), vec![])
        .await
        .unwrap();
    store
        .add("User prefers dark mode".to_string(), vec![])
        .await
        .unwrap();

    let result = execute(
        ToolCallId("s1".to_string()),
        json!({"query": "Rust"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["count"], 1);
            assert_eq!(v["memories"][0]["content"], "User loves Rust programming");
            assert_eq!(v["query"], "Rust");
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_search_no_results_returns_empty() {
    let (store, _tmp) = fresh_store().await;
    store.add("Some memory".to_string(), vec![]).await.unwrap();

    let result = execute(
        ToolCallId("s2".to_string()),
        json!({"query": "xylophone"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["count"], 0);
            assert_eq!(v["memories"].as_array().unwrap().len(), 0);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_search_empty_query_returns_error() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(ToolCallId("s3".to_string()), json!({"query": ""}), &store)
        .await
        .unwrap();

    assert!(result.is_error);
    match result.content {
        ToolContent::Text(msg) => assert!(msg.contains("query is empty")),
        _ => panic!("expected text error"),
    }
}

#[tokio::test]
async fn test_search_whitespace_query_returns_error() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("s4".to_string()),
        json!({"query": "   "}),
        &store,
    )
    .await
    .unwrap();

    assert!(result.is_error);
}

#[tokio::test]
async fn test_search_respects_limit() {
    let (store, _tmp) = fresh_store().await;
    for i in 0..5 {
        store
            .add(format!("Rust tip number {}", i), vec![])
            .await
            .unwrap();
    }

    let result = execute(
        ToolCallId("s5".to_string()),
        json!({"query": "Rust", "limit": 2}),
        &store,
    )
    .await
    .unwrap();

    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["count"], 2);
            assert!(v["memories"].as_array().unwrap().len() <= 2);
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_search_alias_q() {
    let (store, _tmp) = fresh_store().await;
    store
        .add("Dark mode preference".to_string(), vec![])
        .await
        .unwrap();

    let result = execute(ToolCallId("s6".to_string()), json!({"q": "dark"}), &store)
        .await
        .unwrap();

    assert!(!result.is_error, "alias 'q' should work");
}

#[tokio::test]
async fn test_search_alias_term() {
    let (store, _tmp) = fresh_store().await;
    store
        .add("Workflow automation".to_string(), vec![])
        .await
        .unwrap();

    let result = execute(
        ToolCallId("s7".to_string()),
        json!({"term": "Workflow"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error, "alias 'term' should work");
}

#[tokio::test]
async fn test_search_missing_query_returns_parse_error() {
    let (store, _tmp) = fresh_store().await;
    let err = execute(ToolCallId("s8".to_string()), json!({}), &store).await;
    assert!(err.is_err(), "missing query should return ArgsParse error");
}

#[tokio::test]
async fn test_search_with_progress() {
    let (store, _tmp) = fresh_store().await;

    let result = execute_with_progress(
        ToolCallId("s9".to_string()),
        json!({"query": "test"}),
        &store,
        None,
    )
    .await
    .unwrap();

    // No memories, but should succeed without error
    assert!(!result.is_error);
}

#[tokio::test]
async fn test_search_echoes_query_in_output() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("s10".to_string()),
        json!({"query": "  trimmed query  "}),
        &store,
    )
    .await
    .unwrap();

    match result.content {
        ToolContent::Json(v) => {
            // Query should be trimmed in the output
            assert_eq!(v["query"], "trimmed query");
        }
        _ => panic!("expected JSON"),
    }
}
