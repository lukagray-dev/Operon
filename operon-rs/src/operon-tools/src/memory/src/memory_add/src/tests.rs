//! Tests for the memory_add tool.
//!
//! Hey friend! All tests use a tempfile-backed MemoryStore so they never
//! touch the real ~/.operon/memory/memory.db.

use crate::{execute, execute_with_progress};
use operon_context_normalize_tools::{ToolCallId, ToolContent};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;

/// Helper: create a fresh MemoryStore backed by a temp file.
/// The NamedTempFile must be kept alive (returned) or it will be deleted too early.
async fn fresh_store() -> (MemoryStore, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let store = MemoryStore::connect(tmp.path()).await.unwrap();
    (store, tmp)
}

// ─────────────────────────────────────────────────────────────────────────────
// Success paths
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_basic_add_succeeds() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({"content": "User prefers Rust"}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error, "basic add should succeed");
    match result.content {
        ToolContent::Json(v) => {
            let memory = v.get("memory").expect("output must have 'memory'");
            assert_eq!(memory["content"], "User prefers Rust");
            assert!(v.get("total").is_some(), "output must have 'total'");
            assert_eq!(v["total"], 1);
        }
        other => panic!("expected JSON content, got {:?}", other),
    }
}

#[tokio::test]
async fn test_add_with_tags() {
    let (store, _tmp) = fresh_store().await;

    let result = execute(
        ToolCallId("call_2".to_string()),
        json!({"content": "User works in IST timezone", "tags": ["schedule", "preference"]}),
        &store,
    )
    .await
    .unwrap();

    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => {
            let tags = &v["memory"]["tags"];
            assert_eq!(tags, &json!(["schedule", "preference"]));
        }
        other => panic!("expected JSON, got {:?}", other),
    }
}

#[tokio::test]
async fn test_total_increases_with_each_add() {
    let (store, _tmp) = fresh_store().await;

    for i in 1..=3 {
        let result = execute(
            ToolCallId(format!("call_{}", i)),
            json!({"content": format!("Memory {}", i)}),
            &store,
        )
        .await
        .unwrap();

        let total = match &result.content {
            ToolContent::Json(v) => v["total"].as_i64().unwrap(),
            _ => panic!("expected JSON"),
        };
        assert_eq!(total, i as i64, "total should reflect cumulative count");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Alias handling (note, fact, text, memory, info)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_alias_note() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("alias_note".to_string()),
        json!({"note": "Aliased as note"}),
        &store,
    )
    .await
    .unwrap();
    assert!(!result.is_error, "alias 'note' should work");
}

#[tokio::test]
async fn test_alias_fact() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("alias_fact".to_string()),
        json!({"fact": "Aliased as fact"}),
        &store,
    )
    .await
    .unwrap();
    assert!(!result.is_error, "alias 'fact' should work");
}

#[tokio::test]
async fn test_alias_text() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("alias_text".to_string()),
        json!({"text": "Aliased as text"}),
        &store,
    )
    .await
    .unwrap();
    assert!(!result.is_error, "alias 'text' should work");
}

#[tokio::test]
async fn test_alias_info() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("alias_info".to_string()),
        json!({"info": "Aliased as info"}),
        &store,
    )
    .await
    .unwrap();
    assert!(!result.is_error, "alias 'info' should work");
}

// ─────────────────────────────────────────────────────────────────────────────
// Flexible tags deserialization
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_single_string_tag() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("single_tag".to_string()),
        json!({"content": "Some memory", "tags": "single-tag"}),
        &store,
    )
    .await
    .unwrap();
    assert!(!result.is_error, "single string tag should parse as a vec of one");
    match result.content {
        ToolContent::Json(v) => {
            let tags = &v["memory"]["tags"];
            assert_eq!(tags, &json!(["single-tag"]));
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_tag_alias() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("tag_alias".to_string()),
        json!({"content": "Memory with tag alias", "tag": ["one", "two"]}),
        &store,
    )
    .await
    .unwrap();
    assert!(!result.is_error, "alias 'tag' should work for tags field");
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation errors
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_empty_content_returns_error() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("empty_content".to_string()),
        json!({"content": ""}),
        &store,
    )
    .await
    .unwrap();
    assert!(result.is_error, "empty content should fail");
    match result.content {
        ToolContent::Text(msg) => assert!(msg.contains("content is empty")),
        _ => panic!("expected text error"),
    }
}

#[tokio::test]
async fn test_whitespace_only_content_returns_error() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("ws_content".to_string()),
        json!({"content": "   "}),
        &store,
    )
    .await
    .unwrap();
    assert!(result.is_error, "whitespace-only content should fail");
}

#[tokio::test]
async fn test_missing_content_returns_parse_error() {
    let (store, _tmp) = fresh_store().await;
    // Missing the required "content" field — should return Err(ArgsParse).
    let err = execute(
        ToolCallId("missing_content".to_string()),
        json!({"tags": ["something"]}),
        &store,
    )
    .await;
    assert!(err.is_err(), "missing required content should return ArgsParse error");
}

#[tokio::test]
async fn test_whitespace_is_trimmed_before_storage() {
    let (store, _tmp) = fresh_store().await;
    let result = execute(
        ToolCallId("trim_test".to_string()),
        json!({"content": "  Padded content  "}),
        &store,
    )
    .await
    .unwrap();
    assert!(!result.is_error);
    match result.content {
        ToolContent::Json(v) => {
            assert_eq!(v["memory"]["content"], "Padded content",
                "content should be stored trimmed");
        }
        _ => panic!("expected JSON"),
    }
}

#[tokio::test]
async fn test_execute_with_progress_no_progress_emitter() {
    let (store, _tmp) = fresh_store().await;
    // Passing None for the progress emitter should work fine.
    let result = execute_with_progress(
        ToolCallId("prog_test".to_string()),
        json!({"content": "Progress test memory"}),
        &store,
        None,
    )
    .await
    .unwrap();
    assert!(!result.is_error);
}
