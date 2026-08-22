//! Memory Settings Backend — Bridge Commands.

use operon_rs::memory_store::MemoryStore;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Frontend-facing data types (serialised to JSON for the webview)
// ─────────────────────────────────────────────────────────────────────────────

/// A single memory entry as returned to the settings panel.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryEntry {
    /// Unique string id (stringified SQLite ROWID).
    pub id: String,
    /// The memory text content.
    pub content: String,
    /// Zero or more categorisation tags.
    pub tags: Vec<String>,
    /// RFC3339 creation timestamp.
    pub created_at: String,
    /// RFC3339 last-updated timestamp.
    pub updated_at: String,
}

/// Response returned by `memory_list`.
#[derive(Debug, Serialize)]
pub struct MemoryListResponse {
    /// Slice of memories for the requested page, most-recent first.
    pub memories: Vec<MemoryEntry>,
    /// Total count of memories in the store (for pagination display).
    pub total: i64,
}

/// Response returned by `memory_delete`.
#[derive(Debug, Serialize)]
pub struct MemoryDeleteResponse {
    /// The id that was deleted (echoed for frontend confirmation).
    pub id: String,
    /// Count of memories remaining after the deletion.
    pub remaining: i64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Connects to the default store. Returns a user-readable error string on failure.
async fn open_store() -> Result<MemoryStore, String> {
    MemoryStore::connect_default()
        .await
        .map_err(|e| format!("Failed to open memory store: {e}"))
}

/// Maps the library's `Memory` struct to the frontend's `MemoryEntry`.
fn to_entry(m: operon_rs::memory_store::Memory) -> MemoryEntry {
    MemoryEntry {
        id: m.id,
        content: m.content,
        tags: m.tags,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a paginated list of memories, most-recent first.
pub async fn memory_list(limit: usize, offset: usize) -> Result<MemoryListResponse, String> {
    let store = open_store().await?;

    let effective_limit = if limit == 0 { 50 } else { limit };

    let (memories, total) = tokio::try_join!(store.list(effective_limit, offset), store.count())
        .map_err(|e| format!("Store read error: {e}"))?;

    Ok(MemoryListResponse {
        memories: memories.into_iter().map(to_entry).collect(),
        total,
    })
}

/// Creates a new memory with the given content and optional tags.
pub async fn memory_add(content: String, tags: Vec<String>) -> Result<MemoryEntry, String> {
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        return Err("content is empty".to_string());
    }

    let store = open_store().await?;
    let created = store
        .add(trimmed, tags)
        .await
        .map_err(|e| format!("Failed to add memory: {e}"))?;

    Ok(to_entry(created))
}

/// Partially updates an existing memory.
pub async fn memory_edit(
    id: String,
    content: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<MemoryEntry, String> {
    if content.is_none() && tags.is_none() {
        return Err("provide at least one of: content, tags".to_string());
    }

    let validated_content = match content {
        Some(c) => {
            let t = c.trim().to_string();
            if t.is_empty() {
                return Err("content is empty".to_string());
            }
            Some(t)
        }
        None => None,
    };

    let store = open_store().await?;
    let updated = store
        .edit(&id, validated_content, tags)
        .await
        .map_err(|e| format!("Failed to edit memory: {e}"))?;

    updated
        .map(to_entry)
        .ok_or_else(|| format!("memory not found: id '{id}'"))
}

/// Permanently deletes a memory by its id.
pub async fn memory_delete(id: String) -> Result<MemoryDeleteResponse, String> {
    let store = open_store().await?;

    let deleted = store
        .delete(&id)
        .await
        .map_err(|e| format!("Failed to delete memory: {e}"))?;

    if !deleted {
        return Err(format!("memory not found: id '{id}'"));
    }

    let remaining = store
        .count()
        .await
        .map_err(|e| format!("Failed to count memories: {e}"))?;

    Ok(MemoryDeleteResponse { id, remaining })
}
