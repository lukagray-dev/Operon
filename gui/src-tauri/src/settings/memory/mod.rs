//! Memory Settings Backend — Tauri Commands.
//!
//! Hey friend! This module exposes four async Tauri commands that the Memory
//! settings panel (settings.html + memory.ts) calls via `invoke()`:
//!
//!  - `memory_list`   → returns all memories, most-recent first (paginated).
//!  - `memory_add`    → creates a new memory from plain text content + optional tags.
//!  - `memory_edit`   → partially updates an existing memory's content and/or tags.
//!  - `memory_delete` → permanently removes a memory by id and returns new count.
//!
//! All commands operate directly on the default SQLite store at
//! `~/.operon/memory/memory.db` (created if missing on first call).
//! Every invocation calls `MemoryStore::connect_default()` — the pool is cheap
//! to construct and sqlite WAL mode handles concurrent access gracefully.

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
///
/// # Arguments
/// - `limit`  — max entries per page (default: 50 if 0 is passed).
/// - `offset` — number of entries to skip (for pagination).
#[tauri::command]
pub async fn memory_list(limit: usize, offset: usize) -> Result<MemoryListResponse, String> {
    let store = open_store().await?;

    // Resolve sensible defaults — the frontend passes 0 for "use default".
    let effective_limit = if limit == 0 { 50 } else { limit };

    // Fetch the page and the total count concurrently.
    let (memories, total) = tokio::try_join!(
        store.list(effective_limit, offset),
        store.count()
    )
    .map_err(|e| format!("Store read error: {e}"))?;

    Ok(MemoryListResponse {
        memories: memories.into_iter().map(to_entry).collect(),
        total,
    })
}

/// Creates a new memory with the given content and optional tags.
///
/// Returns the fully-populated created entry including its assigned id.
#[tauri::command]
pub async fn memory_add(content: String, tags: Vec<String>) -> Result<MemoryEntry, String> {
    // Validate content before touching the store.
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
///
/// `content` and/or `tags` may be provided; `None` means "leave unchanged".
/// At least one must be `Some`.
#[tauri::command]
pub async fn memory_edit(
    id: String,
    content: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<MemoryEntry, String> {
    // Require at least one field.
    if content.is_none() && tags.is_none() {
        return Err("provide at least one of: content, tags".to_string());
    }

    // Validate content if provided.
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
///
/// Returns the deleted id and the remaining count.
#[tauri::command]
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
