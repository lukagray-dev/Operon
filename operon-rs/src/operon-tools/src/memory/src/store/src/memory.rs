//! Memory data types for operon-tools-memory-store.
//!
//! Hey friend! This module defines two structs:
//!   - `MemoryRow` — the raw SQLite row shape that sqlx maps to (tags as JSON string).
//!   - `Memory`    — the public, model-facing struct with `tags: Vec<String>`.
//!
//! We can't derive `sqlx::FromRow` on `Memory` directly because the `tags` column
//! is stored as a JSON text string in SQLite (e.g. `'["workflow","preference"]'`),
//! and sqlx::FromRow doesn't know how to parse JSON strings into Vec<String>.
//! Instead, we derive it on the internal `MemoryRow` and implement `From<MemoryRow>`
//! to do the JSON deserialization step.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Internal raw row (SQLite → Rust mapping, never exposed publicly)
// ─────────────────────────────────────────────────────────────────────────────

/// Internal SQLite row representation of a memory.
///
/// This matches the schema column-for-column so sqlx's `query_as!()` macro
/// can map rows without manual column extraction. The `tags` field stores the
/// raw JSON string from the database column (e.g. `'["pref","workflow"]'`).
///
/// Never expose this type outside this crate — callers always get `Memory`.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct MemoryRow {
    /// The SQLite integer row id, returned as i64 (SQLite's native integer type).
    /// We convert it to a String for the public `Memory.id` field.
    pub id: i64,

    /// The stored fact/preference/note — raw UTF-8 text, not parsed.
    pub content: String,

    /// JSON-encoded array of tags, e.g. `'["workflow","preference"]'`.
    /// Stored as TEXT in SQLite; we parse it to Vec<String> in From<MemoryRow>.
    pub tags: String,

    /// RFC3339 timestamp string like `"2024-01-01T00:00:00+00:00"`.
    pub created_at: String,

    /// RFC3339 timestamp string like `"2024-01-01T00:00:00+00:00"`.
    pub updated_at: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public Memory struct
// ─────────────────────────────────────────────────────────────────────────────

/// A single stored memory — the model-facing public type.
///
/// Represents one fact, preference, or note the agent has chosen to remember
/// persistently across sessions. Memories are global — not scoped to a session,
/// project, or conversation. They survive process restarts and context compaction.
///
/// The `tags` field is a proper `Vec<String>` (not a JSON string) so tool
/// outputs expose it as a JSON array to the model, which can filter or interpret
/// them naturally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique identifier, auto-assigned by SQLite AUTOINCREMENT.
    ///
    /// Stored as an INTEGER PRIMARY KEY internally but exposed as a String
    /// to the model to stay consistent with how tool IDs work everywhere else
    /// (e.g. `TodoItem.id` is also a String wrapping an integer).
    pub id: String,

    /// The memory content — a fact, preference, or note to remember.
    ///
    /// Examples: "User prefers Rust over Go", "User's work timezone is IST",
    /// "This project uses AGPL-3.0 license".
    pub content: String,

    /// Optional tags for categorization and filtering.
    ///
    /// Examples: `["preference"]`, `["workflow", "git"]`, `[]`.
    /// The model can use these to group related memories or for targeted lookup.
    pub tags: Vec<String>,

    /// RFC3339 timestamp of when this memory was first created.
    ///
    /// Format: `"2024-01-15T10:30:00+00:00"`. Set once on insert; never modified.
    pub created_at: String,

    /// RFC3339 timestamp of when this memory was last updated.
    ///
    /// Equal to `created_at` if the memory was never edited. Updated on every
    /// call to `MemoryStore::edit()` regardless of which fields changed.
    pub updated_at: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion: raw DB row → public type
// ─────────────────────────────────────────────────────────────────────────────

impl From<MemoryRow> for Memory {
    fn from(row: MemoryRow) -> Self {
        // Parse the JSON-encoded tags string from the DB back into a Vec<String>.
        // If the JSON is somehow malformed (shouldn't happen with our controlled inserts),
        // we fall back to an empty vec rather than panicking or failing the whole query.
        let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap_or_default();

        Memory {
            // Convert the i64 row id to a String for the public API.
            id: row.id.to_string(),
            content: row.content,
            tags,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
