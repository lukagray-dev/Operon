//! MemoryStore — the async SQLite-backed global memory store.
//!
//! Hey friend! This is the heart of the memory subsystem. It wraps a `sqlx::SqlitePool`,
//! which is a connection pool that handles all concurrency internally. Because the pool
//! is internally an Arc, you can clone `MemoryStore` cheaply and share it across tasks.
//!
//! Key design decisions:
//! - All methods take `&self` (not `&mut self`) — the pool handles concurrent access.
//! - `Clone` is derived — callers can clone the store and pass it to async tasks.
//! - Schema DDL runs on every connect via `CREATE TABLE IF NOT EXISTS` — idempotent.
//! - FTS5 virtual table keeps full-text search in sync via SQLite AFTER INSERT/UPDATE/DELETE triggers.
//! - Tags are stored as JSON text (e.g. `'["pref","workflow"]'`) and parsed on read.
//! - We use `sqlx::query()` (runtime-checked) rather than `sqlx::query!()` (compile-time
//!   checked) because the compile-time macros require either `DATABASE_URL` in the env or
//!   a pre-generated `.sqlx/` cache directory — neither of which we can guarantee in all
//!   build environments. Runtime-checked queries are simpler to maintain and are just as
//!   safe for this use case.
//! - For `id` lookups: we parse `id: &str` to `i64` before querying. If parsing fails,
//!   we return `Ok(None)` / `Ok(false)` (same as "not found") — callers don't need to
//!   distinguish "bad id format" from "id not in DB".

use crate::error::MemoryStoreError;
use crate::memory::{Memory, MemoryRow};
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Schema DDL — runs on connect, idempotent due to IF NOT EXISTS
// ─────────────────────────────────────────────────────────────────────────────

/// DDL statements to create the memories table, FTS5 virtual table, and sync triggers.
///
/// We define statements individually in an array rather than splitting a string by `;`
/// because SQL triggers contain internal semicolons inside `BEGIN ... END;` blocks.
const SCHEMA_STATEMENTS: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS memories (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        content    TEXT    NOT NULL,
        tags       TEXT    NOT NULL DEFAULT '[]',
        created_at TEXT    NOT NULL,
        updated_at TEXT    NOT NULL
    )"#,
    r#"CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
        content,
        content='memories',
        content_rowid='id'
    )"#,
    r#"CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
        INSERT INTO memories_fts(rowid, content) VALUES (new.id, new.content);
    END"#,
    r#"CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
        INSERT INTO memories_fts(memories_fts, rowid, content) VALUES ('delete', old.id, old.content);
    END"#,
    r#"CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
        INSERT INTO memories_fts(memories_fts, rowid, content) VALUES ('delete', old.id, old.content);
        INSERT INTO memories_fts(rowid, content) VALUES (new.id, new.content);
    END"#,
];

// ─────────────────────────────────────────────────────────────────────────────
// MemoryStore
// ─────────────────────────────────────────────────────────────────────────────

/// Global persistent memory store backed by SQLite.
///
/// Wraps a `sqlx::SqlitePool` (internally an Arc), so cloning is cheap and
/// all methods take `&self` — safe to share across concurrent async tasks
/// without any external locking.
///
/// # Usage
///
/// ```no_run
/// use operon_tools_memory_store::MemoryStore;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Production path — uses ~/.operon/memory/memory.db
/// let store = MemoryStore::connect_default().await?;
///
/// // Test path — use a tempfile
/// // let store = MemoryStore::connect(std::path::Path::new("/tmp/test.db")).await?;
///
/// let memory = store.add("User prefers dark mode".to_string(), vec!["preference".to_string()]).await?;
/// println!("Stored memory id={}", memory.id);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct MemoryStore {
    /// Internal SQLite connection pool. Clone is O(1) — just increments an Arc refcount.
    pool: SqlitePool,
}

impl MemoryStore {
    // ── Construction ─────────────────────────────────────────────────────────

    /// Opens (or creates) the SQLite database at `db_path` and runs schema migrations.
    ///
    /// Creates the parent directory if it doesn't exist, opens or creates the SQLite file,
    /// then runs the schema DDL (all statements are `IF NOT EXISTS` so this is idempotent).
    ///
    /// # Arguments
    /// - `db_path`: Absolute path to the SQLite file. The parent directory will be
    ///   created if missing. Suitable for tests (use a tempfile path) or custom locations.
    ///
    /// # Errors
    /// - `MemoryStoreError::Io` if the parent directory cannot be created.
    /// - `MemoryStoreError::Database` if SQLite connection or schema DDL fails.
    pub async fn connect(db_path: &Path) -> Result<Self, MemoryStoreError> {
        // Create parent dir if missing (e.g. first run, or test with a fresh tempdir).
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Build SQLite connect options using `.filename(path)` rather than a URI string.
        // On Windows, db_path looks like `C:\Users\...\memory.db`. Constructing a URI
        // via `format!("sqlite://{}", path)` breaks: backslashes are not valid URI path
        // separators and the drive letter after `//` is ambiguous with a URI authority.
        // `.filename()` accepts a `Path` directly — no URI encoding, no platform issues.
        let connect_opts = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        // Build a small pool — memory operations are fast so 4 connections is plenty.
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(connect_opts)
            .await?;

        // Run the schema DDL statements. Each statement is IF NOT EXISTS so this is safe
        // to call every time the process starts, not just on first boot.
        for stmt in SCHEMA_STATEMENTS {
            sqlx::query(stmt).execute(&pool).await?;
        }

        Ok(Self { pool })
    }

    /// Opens the database at the default path (`~/.operon/memory/memory.db`).
    ///
    /// Calls `OperonPaths::resolve()` to find the path, then `ensure_dirs_exist()`
    /// to create `~/.operon/memory/` if it doesn't yet exist, then `connect()`.
    ///
    /// This is the production entry point. Tests should use `connect(&tempfile_path)`.
    ///
    /// # Errors
    /// - `MemoryStoreError::Config` if the home directory cannot be resolved.
    /// - `MemoryStoreError::Io` if the memory directory cannot be created.
    /// - `MemoryStoreError::Database` if the SQLite file cannot be opened.
    pub async fn connect_default() -> Result<Self, MemoryStoreError> {
        // resolve() finds ~/.operon and all sub-paths; ensure_dirs_exist() creates
        // ~/.operon/, ~/.operon/workspace/, ~/.operon/sessions/, ~/.operon/memory/.
        let paths = operon_config::OperonPaths::resolve()?;
        paths.ensure_dirs_exist()?;
        Self::connect(&paths.memory_db).await
    }

    // ── Internal helper: map a Row to MemoryRow ───────────────────────────────

    /// Maps a raw sqlx row to a `MemoryRow`.
    ///
    /// We use runtime-checked `sqlx::query()` (no `query!()` macro) to avoid the
    /// compile-time DATABASE_URL requirement. This helper encapsulates the column mapping.
    fn row_to_memory_row(row: &sqlx::sqlite::SqliteRow) -> Result<MemoryRow, sqlx::Error> {
        use sqlx::Row as _;
        Ok(MemoryRow {
            id: row.try_get("id")?,
            content: row.try_get("content")?,
            tags: row.try_get("tags")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    // ── Write operations ─────────────────────────────────────────────────────

    /// Adds a new memory and returns the created `Memory` with its assigned id.
    ///
    /// Both `created_at` and `updated_at` are set to the current UTC time in RFC3339 format.
    /// The tags `Vec<String>` is JSON-encoded before storage.
    ///
    /// # Arguments
    /// - `content`: The memory text. Callers should validate non-empty before calling.
    /// - `tags`: Zero or more tags for categorization. Pass `vec![]` if none.
    ///
    /// # Returns
    /// The fully populated `Memory` struct with the auto-assigned integer id as a String.
    pub async fn add(
        &self,
        content: String,
        tags: Vec<String>,
    ) -> Result<Memory, MemoryStoreError> {
        // Serialize tags to JSON text for storage.
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());

        // RFC3339 timestamp for both created_at and updated_at on creation.
        let now = Utc::now().to_rfc3339();

        // Insert the row. `last_insert_rowid()` gives us the new AUTOINCREMENT id.
        let result = sqlx::query(
            "INSERT INTO memories (content, tags, created_at, updated_at) VALUES (?, ?, ?, ?)"
        )
        .bind(&content)
        .bind(&tags_json)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let row_id = result.last_insert_rowid();

        // Re-fetch the row so we return the exact same data that's stored.
        let row = sqlx::query(
            "SELECT id, content, tags, created_at, updated_at FROM memories WHERE id = ?"
        )
        .bind(row_id)
        .fetch_one(&self.pool)
        .await?;

        let memory_row = Self::row_to_memory_row(&row)?;
        Ok(Memory::from(memory_row))
    }

    /// Applies a partial update to an existing memory.
    ///
    /// Only the fields wrapped in `Some(...)` are changed — `None` means "leave as-is".
    ///
    /// # Arguments
    /// - `id`: String id of the memory. If it cannot be parsed to i64, returns `Ok(None)`.
    /// - `content`: If `Some`, replaces the content. If `None`, content unchanged.
    /// - `tags`: If `Some`, replaces all tags. If `None`, tags unchanged.
    ///
    /// # Returns
    /// - `Ok(Some(Memory))` — the updated memory.
    /// - `Ok(None)` — no row with this id exists (or id is not a valid integer string).
    pub async fn edit(
        &self,
        id: &str,
        content: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<Option<Memory>, MemoryStoreError> {
        // Parse the string id to i64. Invalid format → treat as not found.
        let id_i64: i64 = match id.parse() {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };

        // Fetch the current row first so we can apply partial updates.
        let existing = sqlx::query(
            "SELECT id, content, tags, created_at, updated_at FROM memories WHERE id = ?"
        )
        .bind(id_i64)
        .fetch_optional(&self.pool)
        .await?;

        // Return None if the memory doesn't exist.
        let existing = match existing {
            Some(row) => Self::row_to_memory_row(&row)?,
            None => return Ok(None),
        };

        // Compute the new values: use provided value or fall back to current value.
        let new_content = content.unwrap_or(existing.content);
        let new_tags_json = match tags {
            Some(t) => serde_json::to_string(&t).unwrap_or_else(|_| "[]".to_string()),
            None => existing.tags,
        };
        let new_updated_at = Utc::now().to_rfc3339();

        // Perform the UPDATE — the `memories_au` trigger will re-index the FTS table.
        sqlx::query(
            "UPDATE memories SET content = ?, tags = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&new_content)
        .bind(&new_tags_json)
        .bind(&new_updated_at)
        .bind(id_i64)
        .execute(&self.pool)
        .await?;

        // Re-fetch to return the fully updated struct.
        let updated = sqlx::query(
            "SELECT id, content, tags, created_at, updated_at FROM memories WHERE id = ?"
        )
        .bind(id_i64)
        .fetch_one(&self.pool)
        .await?;

        Ok(Some(Memory::from(Self::row_to_memory_row(&updated)?)))
    }

    /// Deletes a memory by id.
    ///
    /// Returns `true` if a row was deleted, `false` if no row with that id exists.
    /// If `id` cannot be parsed to i64, returns `false` (not an error).
    pub async fn delete(&self, id: &str) -> Result<bool, MemoryStoreError> {
        let id_i64: i64 = match id.parse() {
            Ok(v) => v,
            Err(_) => return Ok(false),
        };

        // The `memories_ad` trigger will remove the deleted row from the FTS index.
        let result = sqlx::query("DELETE FROM memories WHERE id = ?")
            .bind(id_i64)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ── Read operations ───────────────────────────────────────────────────────

    /// Fetches a single memory by id.
    ///
    /// Returns `Ok(None)` if the id doesn't exist or isn't a valid integer string.
    pub async fn get(&self, id: &str) -> Result<Option<Memory>, MemoryStoreError> {
        let id_i64: i64 = match id.parse() {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };

        let row = sqlx::query(
            "SELECT id, content, tags, created_at, updated_at FROM memories WHERE id = ?"
        )
        .bind(id_i64)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(Memory::from(Self::row_to_memory_row(&r)?))),
            None => Ok(None),
        }
    }

    /// Lists memories ordered by `created_at DESC` (most recent first), with pagination.
    ///
    /// # Arguments
    /// - `limit`: Maximum number of rows to return. Use 20 as a sensible default.
    /// - `offset`: Number of rows to skip (for page-based navigation). Start at 0.
    pub async fn list(&self, limit: usize, offset: usize) -> Result<Vec<Memory>, MemoryStoreError> {
        let limit_i64 = limit as i64;
        let offset_i64 = offset as i64;

        let rows = sqlx::query(
            "SELECT id, content, tags, created_at, updated_at FROM memories ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit_i64)
        .bind(offset_i64)
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|r| Ok(Memory::from(Self::row_to_memory_row(r)?)))
            .collect()
    }

    /// Returns the total count of all memories in the store.
    pub async fn count(&self) -> Result<i64, MemoryStoreError> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM memories")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("count")?)
    }

    /// Full-text search over memory content using FTS5.
    ///
    /// Returns memories ranked by FTS5 relevance (BM25, ascending rank = most relevant).
    /// An empty or whitespace-only query returns an empty Vec without erroring.
    ///
    /// # Arguments
    /// - `query`: The search terms. FTS5 MATCH syntax is supported.
    /// - `limit`: Maximum results. Use 10 as a sensible default.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Memory>, MemoryStoreError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let limit_i64 = limit as i64;

        // FTS5 MATCH query with rank-based relevance sorting.
        // We join back to memories to get all columns (memories_fts only stores content + rowid).
        let rows = sqlx::query(
            r#"
            SELECT m.id, m.content, m.tags, m.created_at, m.updated_at
            FROM memories m
            INNER JOIN memories_fts ON memories_fts.rowid = m.id
            WHERE memories_fts MATCH ?
            ORDER BY rank
            LIMIT ?
            "#,
        )
        .bind(trimmed)
        .bind(limit_i64)
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|r| Ok(Memory::from(Self::row_to_memory_row(r)?)))
            .collect()
    }
}
