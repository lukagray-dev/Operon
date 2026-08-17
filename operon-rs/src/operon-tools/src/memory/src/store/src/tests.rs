//! Tests for operon-tools-memory-store.
//!
//! Hey friend! All tests here use isolated temporary SQLite files created by
//! the `tempfile` crate. This means:
//!   1. Tests never touch the real ~/.operon/memory/memory.db.
//!   2. Tests can run in parallel without interfering with each other.
//!   3. Cleanup is automatic when the tempfile drops at the end of the test.
//!
//! Each test creates a fresh store with `MemoryStore::connect(&tmpfile.path())`.

use super::store::MemoryStore;
use tokio::test as async_test;

// ─────────────────────────────────────────────────────────────────────────────
// Helper: create a fresh in-memory-backed store for each test
// ─────────────────────────────────────────────────────────────────────────────

/// Creates an isolated MemoryStore backed by a unique temp file.
/// We use a NamedTempFile and keep it alive via `_guard` so the file isn't
/// deleted before the test finishes.
async fn fresh_store() -> (MemoryStore, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
    let store = MemoryStore::connect(tmp.path())
        .await
        .expect("failed to connect to temp store");
    (store, tmp)
}

// ─────────────────────────────────────────────────────────────────────────────
// Basic CRUD
// ─────────────────────────────────────────────────────────────────────────────

#[async_test]
async fn test_add_returns_memory_with_id() {
    let (store, _tmp) = fresh_store().await;

    let memory = store
        .add("User prefers dark mode".to_string(), vec!["preference".to_string()])
        .await
        .expect("add should succeed");

    // The id must be a non-empty string representing a positive integer.
    assert!(!memory.id.is_empty(), "id must be non-empty");
    assert!(memory.id.parse::<i64>().is_ok(), "id must be a valid integer string");
    assert_eq!(memory.content, "User prefers dark mode");
    assert_eq!(memory.tags, vec!["preference"]);
    // Timestamps must be valid RFC3339 — just check they're non-empty.
    assert!(!memory.created_at.is_empty());
    assert!(!memory.updated_at.is_empty());
    // On creation, both timestamps should be equal.
    assert_eq!(memory.created_at, memory.updated_at);
}

#[async_test]
async fn test_add_multiple_returns_unique_ids() {
    let (store, _tmp) = fresh_store().await;

    let m1 = store.add("Memory one".to_string(), vec![]).await.unwrap();
    let m2 = store.add("Memory two".to_string(), vec![]).await.unwrap();
    let m3 = store.add("Memory three".to_string(), vec![]).await.unwrap();

    // IDs must be unique and increase monotonically (AUTOINCREMENT).
    assert_ne!(m1.id, m2.id);
    assert_ne!(m2.id, m3.id);
    let id1: i64 = m1.id.parse().unwrap();
    let id2: i64 = m2.id.parse().unwrap();
    let id3: i64 = m3.id.parse().unwrap();
    assert!(id1 < id2, "ids should be monotonically increasing");
    assert!(id2 < id3);
}

#[async_test]
async fn test_add_empty_tags() {
    let (store, _tmp) = fresh_store().await;

    let memory = store.add("No tags here".to_string(), vec![]).await.unwrap();
    assert_eq!(memory.tags, Vec::<String>::new(), "empty tags should deserialize to empty vec");
}

#[async_test]
async fn test_get_existing_memory() {
    let (store, _tmp) = fresh_store().await;

    let added = store.add("Lookup test".to_string(), vec!["test".to_string()]).await.unwrap();
    let fetched = store.get(&added.id).await.unwrap();

    assert!(fetched.is_some(), "get should return Some for an existing id");
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id, added.id);
    assert_eq!(fetched.content, "Lookup test");
    assert_eq!(fetched.tags, vec!["test"]);
}

#[async_test]
async fn test_get_nonexistent_returns_none() {
    let (store, _tmp) = fresh_store().await;

    let result = store.get("99999").await.unwrap();
    assert!(result.is_none(), "get on unknown id should return None");
}

#[async_test]
async fn test_get_invalid_id_returns_none() {
    let (store, _tmp) = fresh_store().await;

    // Non-integer id strings should silently return None, not error.
    let result = store.get("not-an-integer").await.unwrap();
    assert!(result.is_none());
}

#[async_test]
async fn test_delete_returns_true_for_existing() {
    let (store, _tmp) = fresh_store().await;

    let memory = store.add("Will be deleted".to_string(), vec![]).await.unwrap();
    let deleted = store.delete(&memory.id).await.unwrap();
    assert!(deleted, "delete should return true for an existing memory");

    // After deletion, get should return None.
    let fetched = store.get(&memory.id).await.unwrap();
    assert!(fetched.is_none(), "get after delete should return None");
}

#[async_test]
async fn test_delete_returns_false_for_missing() {
    let (store, _tmp) = fresh_store().await;

    let result = store.delete("99999").await.unwrap();
    assert!(!result, "delete on unknown id should return false");
}

#[async_test]
async fn test_edit_content_only() {
    let (store, _tmp) = fresh_store().await;

    let original = store.add("Original content".to_string(), vec!["tag1".to_string()]).await.unwrap();
    let updated = store
        .edit(&original.id, Some("Updated content".to_string()), None)
        .await
        .unwrap();

    let updated = updated.expect("edit should return Some for existing id");
    assert_eq!(updated.content, "Updated content", "content should be updated");
    assert_eq!(updated.tags, vec!["tag1"], "tags should be unchanged");
    assert_eq!(updated.created_at, original.created_at, "created_at must never change");
    // updated_at must be >= original (may be equal if the clock didn't tick, but must not regress).
    assert!(updated.updated_at >= original.updated_at);
}

#[async_test]
async fn test_edit_tags_only() {
    let (store, _tmp) = fresh_store().await;

    let original = store.add("Content stays".to_string(), vec!["old".to_string()]).await.unwrap();
    let updated = store
        .edit(&original.id, None, Some(vec!["new1".to_string(), "new2".to_string()]))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(updated.content, "Content stays", "content should be unchanged");
    assert_eq!(updated.tags, vec!["new1", "new2"], "tags should be replaced");
}

#[async_test]
async fn test_edit_nonexistent_returns_none() {
    let (store, _tmp) = fresh_store().await;

    let result = store.edit("99999", Some("new content".to_string()), None).await.unwrap();
    assert!(result.is_none(), "edit on unknown id should return None");
}

#[async_test]
async fn test_edit_invalid_id_returns_none() {
    let (store, _tmp) = fresh_store().await;

    let result = store.edit("not-an-int", Some("new".to_string()), None).await.unwrap();
    assert!(result.is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// List + Pagination
// ─────────────────────────────────────────────────────────────────────────────

#[async_test]
async fn test_list_returns_most_recent_first() {
    let (store, _tmp) = fresh_store().await;

    // Insert in order a, b, c — most recent (c) should come first.
    let a = store.add("Memory A".to_string(), vec![]).await.unwrap();
    let b = store.add("Memory B".to_string(), vec![]).await.unwrap();
    let c = store.add("Memory C".to_string(), vec![]).await.unwrap();

    let list = store.list(10, 0).await.unwrap();
    assert_eq!(list.len(), 3);
    // Most recent first (by created_at DESC).
    assert_eq!(list[0].id, c.id);
    assert_eq!(list[1].id, b.id);
    assert_eq!(list[2].id, a.id);
}

#[async_test]
async fn test_list_pagination_limit() {
    let (store, _tmp) = fresh_store().await;

    for i in 0..5 {
        store.add(format!("Memory {}", i), vec![]).await.unwrap();
    }

    let page1 = store.list(3, 0).await.unwrap();
    assert_eq!(page1.len(), 3, "limit=3 should return 3 items");

    let page2 = store.list(3, 3).await.unwrap();
    assert_eq!(page2.len(), 2, "offset=3 with 5 total should return 2 items");
}

#[async_test]
async fn test_list_empty_store() {
    let (store, _tmp) = fresh_store().await;

    let list = store.list(20, 0).await.unwrap();
    assert!(list.is_empty(), "fresh store should return empty list");
}

#[async_test]
async fn test_count_increases_on_add() {
    let (store, _tmp) = fresh_store().await;

    assert_eq!(store.count().await.unwrap(), 0);
    store.add("One".to_string(), vec![]).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 1);
    store.add("Two".to_string(), vec![]).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 2);
}

#[async_test]
async fn test_count_decreases_on_delete() {
    let (store, _tmp) = fresh_store().await;

    let m = store.add("Will be deleted".to_string(), vec![]).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 1);
    store.delete(&m.id).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// FTS5 Search
// ─────────────────────────────────────────────────────────────────────────────

#[async_test]
async fn test_search_finds_matching_content() {
    let (store, _tmp) = fresh_store().await;

    store.add("User loves Rust programming".to_string(), vec![]).await.unwrap();
    store.add("User prefers dark mode themes".to_string(), vec![]).await.unwrap();
    store.add("Project uses AGPL license".to_string(), vec![]).await.unwrap();

    let results = store.search("Rust", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "User loves Rust programming");
}

#[async_test]
async fn test_search_empty_query_returns_empty() {
    let (store, _tmp) = fresh_store().await;

    store.add("Some memory".to_string(), vec![]).await.unwrap();

    let results = store.search("", 10).await.unwrap();
    assert!(results.is_empty(), "empty query should return empty vec, not all memories");

    let results = store.search("   ", 10).await.unwrap();
    assert!(results.is_empty(), "whitespace-only query should also return empty vec");
}

#[async_test]
async fn test_search_respects_limit() {
    let (store, _tmp) = fresh_store().await;

    for i in 0..5 {
        store.add(format!("Rust tip number {}", i), vec![]).await.unwrap();
    }

    let results = store.search("Rust", 3).await.unwrap();
    assert!(results.len() <= 3, "search should respect the limit parameter");
}

#[async_test]
async fn test_search_no_results_for_unknown_term() {
    let (store, _tmp) = fresh_store().await;

    store.add("I like coffee".to_string(), vec![]).await.unwrap();

    let results = store.search("xylophone", 10).await.unwrap();
    assert!(results.is_empty(), "search for unknown term should return empty vec");
}

#[async_test]
async fn test_search_after_delete_does_not_return_deleted() {
    let (store, _tmp) = fresh_store().await;

    let m = store.add("Deleted memory about Rust".to_string(), vec![]).await.unwrap();
    store.add("Surviving memory about Rust".to_string(), vec![]).await.unwrap();
    store.delete(&m.id).await.unwrap();

    let results = store.search("Rust", 10).await.unwrap();
    assert_eq!(results.len(), 1, "deleted memory should not appear in search results");
    assert_eq!(results[0].content, "Surviving memory about Rust");
}

#[async_test]
async fn test_search_after_edit_indexes_new_content() {
    let (store, _tmp) = fresh_store().await;

    let m = store.add("Old content about coffee".to_string(), vec![]).await.unwrap();

    // Edit to change content to tea — search for "coffee" should no longer find it.
    store.edit(&m.id, Some("New content about tea".to_string()), None).await.unwrap();

    let coffee_results = store.search("coffee", 10).await.unwrap();
    assert!(coffee_results.is_empty(), "FTS index should be updated after edit; old term not found");

    let tea_results = store.search("tea", 10).await.unwrap();
    assert_eq!(tea_results.len(), 1, "FTS index should return updated content");
}
