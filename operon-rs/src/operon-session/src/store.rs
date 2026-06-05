// store.rs — SQLite-backed session persistence for operon-session.
//
// This module provides `SessionStore`, which persists session metadata and
// per-turn message arrays to a SQLite file using sqlx. The schema is minimal:
//
//   sessions — one row per agent session (id, metadata, timestamps)
//   turns    — one row per completed turn (messages JSON, token count)
//
// Design notes:
//   - The database is created (including all directories) on first open.
//   - Schema migration is idempotent via CREATE TABLE IF NOT EXISTS.
//   - Messages are serialized as a JSON blob (serde_json::to_string).
//   - sqlx query macros are not used here so no compile-time DB connection is
//     needed — plain `query` / `query_as` with bind() is used instead.
//   - All public methods are async and return SessionError on failure.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use operon_context_normalize_messages::ConversationMessage;

use crate::error::SessionError;

// ─────────────────────────────────────────────────────────────────────────────
// SessionStore
// ─────────────────────────────────────────────────────────────────────────────

/// SQLite-backed store for persisting session metadata and turn history.
///
/// One instance per agent session. Opened by `SessionRunner::new` when
/// `SessionConfig::store_path` is `Some`. All operations are idempotent
/// with respect to the database schema (via `CREATE TABLE IF NOT EXISTS`).
pub struct SessionStore {
    /// The underlying connection pool. SQLite connection pools are limited to
    /// max_connections=1 in WAL mode to avoid contention.
    pool: SqlitePool,
}

impl SessionStore {
    /// Open (or create) the SQLite database at the given path.
    ///
    /// The file and any parent directories are created automatically.
    /// On first open, the schema is initialized via `migrate()`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on any connection or schema failure.
    pub async fn open(path: &Path) -> Result<Self, SessionError> {
        // Ensure the parent directory exists so SQLite can create the file.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SessionError::Store(format!("Failed to create store directory: {e}"))
            })?;
        }

        // Build connection options — create the file if it doesn't exist yet.
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            // WAL mode is strongly preferred for concurrent read access and
            // crash safety. The session runner only writes, but TUI may read.
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        // Use a single connection pool (max 1) — SQLite writes are serialized
        // by design and we never need concurrent writers.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| SessionError::Store(format!("Failed to open store: {e}")))?;

        let store = Self { pool };

        // Ensure schema is up to date (idempotent).
        // migrate() is an associated function (takes &SqlitePool, not &self).
        SessionStore::migrate(&store.pool).await?;

        Ok(store)
    }

    /// Ensure schema exists. Idempotent — safe to call on every open.
    ///
    /// Creates the `sessions` and `turns` tables if they do not already exist.
    async fn migrate(pool: &SqlitePool) -> Result<(), SessionError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id          TEXT    PRIMARY KEY,
                created_at  INTEGER NOT NULL,
                workspace   TEXT    NOT NULL,
                model_id    TEXT    NOT NULL,
                provider    TEXT    NOT NULL
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| SessionError::Store(format!("Failed to create sessions table: {e}")))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS turns (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id    TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                turn_index    INTEGER NOT NULL,
                messages_json TEXT    NOT NULL,
                token_count   INTEGER,
                created_at    INTEGER NOT NULL
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| SessionError::Store(format!("Failed to create turns table: {e}")))?;

        Ok(())
    }

    /// Persist a new session record in the `sessions` table.
    ///
    /// Call once after `SessionRunner::new` generates a session ID.
    /// `created_at` is stored as a Unix epoch timestamp in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] if the INSERT fails (e.g. duplicate ID).
    pub async fn create_session(
        &self,
        session_id: &str,
        workspace: &str,
        model_id: &str,
        provider: &str,
    ) -> Result<(), SessionError> {
        // Use the current wall-clock time as the creation timestamp.
        let created_at = unix_timestamp_secs();

        sqlx::query(
            "INSERT INTO sessions (id, created_at, workspace, model_id, provider) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(session_id)
        .bind(created_at as i64)
        .bind(workspace)
        .bind(model_id)
        .bind(provider)
        .execute(&self.pool)
        .await
        .map_err(|e| SessionError::Store(format!("Failed to create session record: {e}")))?;

        Ok(())
    }

    /// Save the message array for one completed turn.
    ///
    /// `messages` is serialized to JSON and stored in the `messages_json`
    /// column. `token_count` is optional — set to the input token count from
    /// the API response when available, or `None` if the turn failed before
    /// usage data was received.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on serialization or DB write failure.
    pub async fn save_turn(
        &self,
        session_id: &str,
        turn_index: usize,
        messages: &[ConversationMessage],
        token_count: Option<usize>,
    ) -> Result<(), SessionError> {
        // Serialize the entire message array to a compact JSON string.
        let messages_json = serde_json::to_string(messages)
            .map_err(|e| SessionError::Store(format!("Failed to serialize messages: {e}")))?;

        let created_at = unix_timestamp_secs();

        sqlx::query(
            "INSERT INTO turns (session_id, turn_index, messages_json, token_count, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(session_id)
        .bind(turn_index as i64)
        .bind(&messages_json)
        .bind(token_count.map(|n| n as i64))
        .bind(created_at as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| SessionError::Store(format!("Failed to save turn: {e}")))?;

        Ok(())
    }

    /// Load all turns for a session in ascending turn_index order.
    ///
    /// Returns a `Vec` where each element is the deserialized `Vec<ConversationMessage>`
    /// for that turn. Empty if no turns have been saved yet.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on DB query or JSON deserialization failure.
    pub async fn load_turns(
        &self,
        session_id: &str,
    ) -> Result<Vec<Vec<ConversationMessage>>, SessionError> {
        // Fetch all turn rows for this session, sorted by turn_index ascending.
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT messages_json FROM turns WHERE session_id = ? ORDER BY turn_index ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SessionError::Store(format!("Failed to load turns: {e}")))?;

        // Deserialize each row's JSON blob back into the canonical message type.
        rows.into_iter()
            .map(|(json,)| {
                serde_json::from_str::<Vec<ConversationMessage>>(&json)
                    .map_err(|e| SessionError::Store(format!("Failed to deserialize turn: {e}")))
            })
            .collect()
    }

    /// List all sessions in the store with their metadata.
    ///
    /// Ordered by `created_at` ascending (oldest first).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on DB query failure.
    pub async fn list_sessions(&self) -> Result<Vec<SessionRow>, SessionError> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            "SELECT id, created_at, workspace, model_id, provider FROM sessions ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SessionError::Store(format!("Failed to list sessions: {e}")))?;

        Ok(rows)
    }

    /// Get the token count of the last recorded turn for a session.
    /// Used when resuming a session to initialize the token tracker's context estimate.
    pub async fn get_last_token_count(&self, session_id: &str) -> Result<Option<usize>, SessionError> {
        let row: Option<(Option<i64>,)> = sqlx::query_as(
            "SELECT token_count FROM turns WHERE session_id = ? ORDER BY turn_index DESC LIMIT 1"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SessionError::Store(format!("Failed to query last token count: {e}")))?;

        Ok(row.and_then(|(tc,)| tc.map(|val| val as usize)))
    }

    /// Extract the first user message text to use as the chat title.
    pub async fn get_first_user_message_text(&self, session_id: &str) -> Result<Option<String>, SessionError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT messages_json FROM turns WHERE session_id = ? AND turn_index = 0 LIMIT 1"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SessionError::Store(format!("Failed to query first turn: {e}")))?;

        if let Some((json,)) = row {
            if let Ok(messages) = serde_json::from_str::<Vec<ConversationMessage>>(&json) {
                for msg in messages {
                    if msg.role == operon_context_normalize_messages::MessageRole::User {
                        for block in msg.content {
                            if let operon_context_normalize_messages::ContentBlock::Text(text) = block {
                                return Ok(Some(text));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SessionRow
// ─────────────────────────────────────────────────────────────────────────────

/// A single row from the `sessions` table.
///
/// Used by [`SessionStore::list_sessions`] to enumerate persisted sessions.
#[derive(Debug, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    /// Unix epoch timestamp (seconds) when the session was created.
    pub created_at: i64,
    pub workspace: String,
    pub model_id: String,
    pub provider: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the current wall-clock time as a Unix epoch timestamp in seconds.
fn unix_timestamp_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize_messages::{ContentBlock, ConversationMessage};

    /// Open an in-memory SQLite store for testing.
    /// sqlx supports the ":memory:" path for ephemeral databases.
    async fn memory_store() -> SessionStore {
        let path = std::path::Path::new(":memory:");
        SessionStore::open(path)
            .await
            .expect("Failed to open in-memory store")
    }

    /// Build a minimal conversation for testing.
    fn make_messages(text: &str) -> Vec<ConversationMessage> {
        vec![ConversationMessage::user(vec![ContentBlock::Text(
            text.to_string(),
        )])]
    }

    #[tokio::test]
    async fn create_session_and_list_it() {
        // Verify that a session created via create_session appears in list_sessions.
        let store = memory_store().await;

        store
            .create_session("session-1", "/workspace", "claude-sonnet-4", "Anthropic")
            .await
            .expect("create_session should succeed");

        let sessions = store
            .list_sessions()
            .await
            .expect("list_sessions should succeed");

        assert_eq!(sessions.len(), 1, "Should have exactly one session");
        assert_eq!(sessions[0].id, "session-1");
        assert_eq!(sessions[0].workspace, "/workspace");
        assert_eq!(sessions[0].model_id, "claude-sonnet-4");
        assert_eq!(sessions[0].provider, "Anthropic");
    }

    #[tokio::test]
    async fn save_turn_and_load_it_back() {
        // Full round-trip: save a turn's messages and verify they deserialize correctly.
        let store = memory_store().await;

        store
            .create_session("session-rt", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let messages = make_messages("Hello, agent!");
        store
            .save_turn("session-rt", 0, &messages, Some(512))
            .await
            .expect("save_turn");

        let loaded = store.load_turns("session-rt").await.expect("load_turns");

        assert_eq!(loaded.len(), 1, "Should have exactly one turn");
        assert_eq!(
            loaded[0], messages,
            "Loaded messages must match saved messages"
        );
    }

    #[tokio::test]
    async fn load_turns_returns_empty_for_new_session() {
        // A freshly created session with no saved turns should return an empty vec.
        let store = memory_store().await;

        store
            .create_session("session-empty", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let loaded = store.load_turns("session-empty").await.expect("load_turns");
        assert!(
            loaded.is_empty(),
            "No turns saved — should return empty vec"
        );
    }

    #[tokio::test]
    async fn multiple_turns_are_ordered_correctly() {
        // Turns must come back in turn_index ascending order, regardless of insertion order.
        let store = memory_store().await;

        store
            .create_session("session-order", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let turn0 = make_messages("Turn zero");
        let turn1 = make_messages("Turn one");
        let turn2 = make_messages("Turn two");

        store
            .save_turn("session-order", 0, &turn0, None)
            .await
            .unwrap();
        store
            .save_turn("session-order", 1, &turn1, None)
            .await
            .unwrap();
        store
            .save_turn("session-order", 2, &turn2, None)
            .await
            .unwrap();

        let loaded = store.load_turns("session-order").await.unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0], turn0, "Turn 0 should be first");
        assert_eq!(loaded[1], turn1, "Turn 1 should be second");
        assert_eq!(loaded[2], turn2, "Turn 2 should be third");
    }

    #[tokio::test]
    async fn save_turn_with_none_token_count() {
        // token_count is optional — persisting None should not cause an error.
        let store = memory_store().await;

        store
            .create_session("session-no-tokens", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let messages = make_messages("No token count here");
        store
            .save_turn("session-no-tokens", 0, &messages, None)
            .await
            .expect("save_turn with None token_count should succeed");

        let loaded = store.load_turns("session-no-tokens").await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0], messages);
    }
}
