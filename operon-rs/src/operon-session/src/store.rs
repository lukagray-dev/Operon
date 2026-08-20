// store.rs — JSON-backed session persistence for operon-session.
//
// Hey there! This module provides `SessionStore`, which persists session metadata and
// per-turn message lists directly to a local JSON file on the user's hard drive.
//
// In the old days, we used SQLite to do this. But SQL databases are binary, hard to inspect,
// and can lead to tricky database lock or connection issues. Moving to simple, human-readable
// JSON files makes debugging an absolute breeze! You can just open the files in VS Code and
// see exactly what has been saved.
//
// Design notes:
//   - Each agent session gets its own JSON file (e.g. `~/.operon/sessions/<session_id>.json`).
//   - The database file and any parent directories are automatically created on first open.
//   - The JSON structure contains the session metadata (ID, workspace, model details) and a
//     list of all conversation turns.
//   - Reading and writing are done via standard file operations and `serde_json`.

use std::path::{Path, PathBuf};

use operon_context::ConversationMessage;
use operon_tools::TodoItem;
use serde::{Deserialize, Serialize};

use crate::error::SessionError;

// ─────────────────────────────────────────────────────────────────────────────
// Data Schemas
// ─────────────────────────────────────────────────────────────────────────────

/// The root struct representing a session's persisted data in JSON format.
/// Having all data in one struct makes it extremely simple to read and write
/// in a single atomic JSON operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJson {
    /// Unique identifier for this session.
    pub id: String,
    /// Unix epoch timestamp (seconds) when the session was created.
    pub created_at: i64,
    /// Absolute path of the workspace folder.
    pub workspace: String,
    /// Identifier of the model (e.g. llama-3.1-8b-instant).
    pub model_id: String,
    /// Provider name (e.g. Groq, OpenAI, etc.).
    pub provider: String,
    /// Ordered list of all conversation turns.
    pub turns: Vec<TurnJson>,
    /// Session-scoped todo items created and managed during this session.
    /// Defaulted to empty vector for backward compatibility with older session files.
    #[serde(default)]
    pub todos: Vec<TodoItem>,
}

/// A single interaction turn containing the conversation messages list
/// and the model's token count for that turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnJson {
    /// 0-based index of this turn.
    pub turn_index: usize,
    /// The full conversation messages list up to this turn.
    pub messages: Vec<ConversationMessage>,
    /// Estimated or exact number of tokens in the context window.
    pub token_count: Option<usize>,
    /// Unix epoch timestamp (seconds) when this turn was recorded.
    pub created_at: i64,
}

/// A single row representing session metadata, returned by list_sessions.
/// Derived from SessionRow to stay fully compatible with existing callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    pub id: String,
    pub created_at: i64,
    pub workspace: String,
    pub model_id: String,
    pub provider: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// SessionStore
// ─────────────────────────────────────────────────────────────────────────────

/// JSON-backed store for persisting session metadata and turn history.
///
/// One instance per agent session. Opened by `SessionRunner::new` when
/// `SessionConfig::store_path` is `Some`.
pub struct SessionStore {
    /// Path to the JSON file where all data for this session is stored.
    path: PathBuf,
}

impl SessionStore {
    /// Open (or prepare) the JSON file store at the given path.
    ///
    /// The parent directories are created automatically.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on any directory creation failure.
    pub async fn open(path: &Path) -> Result<Self, SessionError> {
        // Hey buddy! First we check if the parent folder of this file exists.
        // If it doesn't, we create it recursively so that writing the file later won't fail.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SessionError::Store(format!("Failed to create store directory: {e}"))
            })?;
        }

        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Persist a new session record (metadata) by creating/overwriting the JSON file.
    ///
    /// Call once after `SessionRunner::new` generates a session ID.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] if serialization or file writing fails.
    pub async fn create_session(
        &self,
        session_id: &str,
        workspace: &str,
        model_id: &str,
        provider: &str,
    ) -> Result<(), SessionError> {
        let created_at = unix_timestamp_secs() as i64;

        // Build the session structure with empty turns and todos lists initially.
        let session = SessionJson {
            id: session_id.to_string(),
            created_at,
            workspace: workspace.to_string(),
            model_id: model_id.to_string(),
            provider: provider.to_string(),
            turns: Vec::new(),
            todos: Vec::new(),
        };

        // Convert it to a pretty JSON string. Pretty formatting is great for debugging!
        let json_str = serde_json::to_string_pretty(&session)
            .map_err(|e| SessionError::Store(format!("Failed to serialize session: {e}")))?;

        // Write the JSON string to our file path.
        std::fs::write(&self.path, json_str)
            .map_err(|e| SessionError::Store(format!("Failed to write session file: {e}")))?;

        Ok(())
    }

    /// Save the message list for one completed turn.
    ///
    /// If the turn already exists (same turn_index), it is updated. Otherwise, it is appended.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on serialization or file write failure.
    pub async fn save_turn(
        &self,
        session_id: &str,
        turn_index: usize,
        messages: &[ConversationMessage],
        token_count: Option<usize>,
    ) -> Result<(), SessionError> {
        // Hey friend! Let's read the current file. If it doesn't exist, we'll
        // gracefully create a new empty session skeleton.
        let mut session = if self.path.exists() {
            let file_content = std::fs::read_to_string(&self.path)
                .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
            serde_json::from_str::<SessionJson>(&file_content)
                .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?
        } else {
            SessionJson {
                id: session_id.to_string(),
                created_at: unix_timestamp_secs() as i64,
                workspace: String::new(),
                model_id: String::new(),
                provider: String::new(),
                turns: Vec::new(),
                todos: Vec::new(),
            }
        };

        let turn = TurnJson {
            turn_index,
            messages: messages.to_vec(),
            token_count,
            created_at: unix_timestamp_secs() as i64,
        };

        // If a turn with the same index already exists, replace it. Otherwise, append.
        if let Some(pos) = session
            .turns
            .iter()
            .position(|t| t.turn_index == turn_index)
        {
            session.turns[pos] = turn;
        } else {
            session.turns.push(turn);
        }

        // Keep turns sorted by index, just to be clean and deterministic.
        session.turns.sort_by_key(|t| t.turn_index);

        // Serialize the whole session data structure back to disk.
        let json_str = serde_json::to_string_pretty(&session)
            .map_err(|e| SessionError::Store(format!("Failed to serialize session: {e}")))?;
        std::fs::write(&self.path, json_str)
            .map_err(|e| SessionError::Store(format!("Failed to write session file: {e}")))?;

        Ok(())
    }

    /// Apply compaction to the persisted session turns.
    ///
    /// When context compaction runs, older turns are summarized into a fresh system snapshot
    /// and summary message while preserving recent turns. This method resets `session.turns`
    /// to the compacted baseline (turn 0) so that subsequent turns and future sessions
    /// reload only the compacted context rather than uncompacted older messages.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read/write or JSON serialization failure.
    pub async fn apply_compaction(
        &self,
        session_id: &str,
        compacted_baseline: &[ConversationMessage],
        token_count: Option<usize>,
    ) -> Result<(), SessionError> {
        let mut session = if self.path.exists() {
            let file_content = std::fs::read_to_string(&self.path)
                .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
            serde_json::from_str::<SessionJson>(&file_content)
                .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?
        } else {
            SessionJson {
                id: session_id.to_string(),
                created_at: unix_timestamp_secs() as i64,
                workspace: String::new(),
                model_id: String::new(),
                provider: String::new(),
                turns: Vec::new(),
                todos: Vec::new(),
            }
        };

        let baseline_turn = TurnJson {
            turn_index: 0,
            messages: compacted_baseline.to_vec(),
            token_count,
            created_at: unix_timestamp_secs() as i64,
        };

        session.turns = vec![baseline_turn];

        let json_str = serde_json::to_string_pretty(&session)
            .map_err(|e| SessionError::Store(format!("Failed to serialize session: {e}")))?;
        std::fs::write(&self.path, json_str)
            .map_err(|e| SessionError::Store(format!("Failed to write session file: {e}")))?;

        Ok(())
    }

    /// Save the current list of session todos to the session JSON file.
    ///
    /// Hey friend! Whenever the model creates, updates, or deletes todo items,
    /// this method ensures the latest task list is immediately serialized to disk.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read/write or JSON serialization failure.
    pub async fn save_todos(
        &self,
        session_id: &str,
        todos: &[TodoItem],
    ) -> Result<(), SessionError> {
        let mut session = if self.path.exists() {
            let file_content = std::fs::read_to_string(&self.path)
                .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
            serde_json::from_str::<SessionJson>(&file_content)
                .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?
        } else {
            SessionJson {
                id: session_id.to_string(),
                created_at: unix_timestamp_secs() as i64,
                workspace: String::new(),
                model_id: String::new(),
                provider: String::new(),
                turns: Vec::new(),
                todos: Vec::new(),
            }
        };

        session.todos = todos.to_vec();

        let json_str = serde_json::to_string_pretty(&session)
            .map_err(|e| SessionError::Store(format!("Failed to serialize session: {e}")))?;
        std::fs::write(&self.path, json_str)
            .map_err(|e| SessionError::Store(format!("Failed to write session file: {e}")))?;

        Ok(())
    }

    /// Load all todo items associated with this session.
    ///
    /// Returns an empty vector if the session file does not exist or has no todos.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or JSON deserialization failure.
    pub async fn load_todos(
        &self,
        _session_id: &str,
    ) -> Result<Vec<TodoItem>, SessionError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        Ok(session.todos)
    }

    /// Truncate all saved turns with turn_index >= keep_turns_count for a session.
    ///
    /// This removes all saved turns starting from `keep_turns_count` onwards,
    /// enabling editing a previous user turn and continuing the conversation from there.
    /// Note: session todos are preserved during truncation!
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read/write or JSON serialization failure.
    pub async fn truncate_turns(
        &self,
        _session_id: &str,
        keep_turns_count: usize,
    ) -> Result<(), SessionError> {
        if !self.path.exists() {
            return Ok(());
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let mut session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        session.turns.retain(|t| t.turn_index < keep_turns_count);

        let json_str = serde_json::to_string_pretty(&session)
            .map_err(|e| SessionError::Store(format!("Failed to serialize session: {e}")))?;
        std::fs::write(&self.path, json_str)
            .map_err(|e| SessionError::Store(format!("Failed to write session file: {e}")))?;

        Ok(())
    }

    /// Load all turns for a session in ascending turn_index order.
    ///
    /// Returns a `Vec` where each element is the `Vec<ConversationMessage>` for that turn.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or JSON deserialization failure.
    pub async fn load_turns(
        &self,
        _session_id: &str,
    ) -> Result<Vec<Vec<ConversationMessage>>, SessionError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let mut session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        // Make sure the turns are sorted in order!
        session.turns.sort_by_key(|t| t.turn_index);

        Ok(session.turns.into_iter().map(|t| t.messages).collect())
    }

    /// Load all turns for a session with their created_at timestamps in ascending order.
    ///
    /// Returns a `Vec<(i64, Vec<ConversationMessage>)>` where the first element is the
    /// Unix timestamp `created_at` for that turn, and the second is the messages list.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or JSON deserialization failure.
    pub async fn load_turns_with_timestamps(
        &self,
        _session_id: &str,
    ) -> Result<Vec<(i64, Vec<ConversationMessage>)>, SessionError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let mut session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        session.turns.sort_by_key(|t| t.turn_index);

        Ok(session
            .turns
            .into_iter()
            .map(|t| (t.created_at, t.messages))
            .collect())
    }

    /// Load all messages across all turns in chronological order.
    ///
    /// Flattens the per-turn message vectors into a single combined conversation vector.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or JSON deserialization failure.
    pub async fn load_full_history(
        &self,
        session_id: &str,
    ) -> Result<Vec<ConversationMessage>, SessionError> {
        let turns = self.load_turns(session_id).await?;
        Ok(turns.into_iter().flatten().collect())
    }

    /// List all sessions in the store with their metadata.
    ///
    /// Since each JSON file represents exactly one session, this returns a single row.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or parsing failure.
    pub async fn list_sessions(&self) -> Result<Vec<SessionRow>, SessionError> {
        // Hey buddy! If the file is not there, we can't list any session, so we return empty.
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        let row = SessionRow {
            id: session.id,
            created_at: session.created_at,
            workspace: session.workspace,
            model_id: session.model_id,
            provider: session.provider,
        };

        Ok(vec![row])
    }

    /// Get the token count of the last recorded turn for a session.
    /// Used when resuming a session to initialize the token tracker's context estimate.
    pub async fn get_last_token_count(
        &self,
        _session_id: &str,
    ) -> Result<Option<usize>, SessionError> {
        if !self.path.exists() {
            return Ok(None);
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        // Find the turn with the highest turn_index to grab the token count.
        let last_turn = session.turns.iter().max_by_key(|t| t.turn_index);
        Ok(last_turn.and_then(|t| t.token_count))
    }

    /// Extract the first user message text to use as the chat title.
    pub async fn get_first_user_message_text(
        &self,
        _session_id: &str,
    ) -> Result<Option<String>, SessionError> {
        if !self.path.exists() {
            return Ok(None);
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        // Search turns in ascending order for the first user text message.
        for turn in &session.turns {
            for msg in &turn.messages {
                if msg.role == operon_context::MessageRole::User {
                    for block in &msg.content {
                        if let operon_context::ContentBlock::Text(text) = block {
                            return Ok(Some(text.clone()));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
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
    use operon_context::{ContentBlock, ConversationMessage};
    use operon_tools::{TodoPriority, TodoStatus};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Hey buddy! Since unit tests in Rust run concurrently on multiple threads,
    // we use this static atomic counter to generate a unique suffix for each test's
    // temporary file. This completely prevents different tests from writing to the
    // same file at the same time!
    static TEST_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Build a temporary file path for tests.
    async fn temp_store_path() -> PathBuf {
        let temp_dir = std::env::temp_dir();
        // Increment the counter atomically. Ordering::SeqCst ensures sequential consistency
        // across all processor cores.
        let count = TEST_FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let session_id = format!("test-session-{}-{}", unix_timestamp_secs(), count);
        temp_dir.join(format!("{}.json", session_id))
    }

    /// Build a minimal conversation for testing.
    fn make_messages(text: &str) -> Vec<ConversationMessage> {
        vec![ConversationMessage::user(vec![ContentBlock::Text(
            text.to_string(),
        )])]
    }

    #[tokio::test]
    async fn create_session_and_list_it() {
        // Let's test that creating a session writes the correct metadata and lists it correctly.
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

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

        // Clean up our temporary file!
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn save_and_load_todos() {
        // Hey friend! Let's test saving and loading todo items for a session.
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-todos", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let initial_todos = store.load_todos("session-todos").await.expect("load_todos");
        assert!(initial_todos.is_empty(), "Initial todos should be empty");

        let todos = vec![
            TodoItem {
                id: "1".to_string(),
                content: "Implement feature X".to_string(),
                status: TodoStatus::Pending,
                priority: TodoPriority::High,
            },
            TodoItem {
                id: "2".to_string(),
                content: "Write unit tests".to_string(),
                status: TodoStatus::InProgress,
                priority: TodoPriority::Medium,
            },
        ];

        store
            .save_todos("session-todos", &todos)
            .await
            .expect("save_todos");

        let loaded = store
            .load_todos("session-todos")
            .await
            .expect("load_todos after save");

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "1");
        assert_eq!(loaded[0].content, "Implement feature X");
        assert_eq!(loaded[0].status, TodoStatus::Pending);
        assert_eq!(loaded[0].priority, TodoPriority::High);
        assert_eq!(loaded[1].id, "2");
        assert_eq!(loaded[1].status, TodoStatus::InProgress);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn backward_compatibility_when_todos_key_is_missing() {
        // Hey friend! Older JSON files don't have the "todos" field.
        // Let's verify serde(default) treats missing todos as an empty list without error!
        let path = temp_store_path().await;
        let old_json = r#"{
            "id": "old-session",
            "created_at": 1700000000,
            "workspace": "/ws",
            "model_id": "model",
            "provider": "provider",
            "turns": []
        }"#;

        std::fs::write(&path, old_json).expect("write old json");
        let store = SessionStore::open(&path).await.expect("open store");

        let loaded = store.load_todos("old-session").await.expect("load_todos");
        assert!(loaded.is_empty(), "Missing todos key should default to empty vector");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn save_turn_and_load_it_back() {
        // Full round-trip: save a turn's messages and verify they deserialize correctly.
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

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

        let last_token = store.get_last_token_count("session-rt").await.unwrap();
        assert_eq!(last_token, Some(512));

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn load_turns_returns_empty_for_new_session() {
        // A freshly created session with no saved turns should return an empty vec.
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-empty", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let loaded = store.load_turns("session-empty").await.expect("load_turns");
        assert!(
            loaded.is_empty(),
            "No turns saved — should return empty vec"
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn multiple_turns_are_ordered_correctly() {
        // Turns must come back in turn_index ascending order, regardless of insertion order.
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-order", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let turn0 = make_messages("Turn zero");
        let turn1 = make_messages("Turn one");
        let turn2 = make_messages("Turn two");

        // Save in non-sequential order to test sorting
        store
            .save_turn("session-order", 2, &turn2, None)
            .await
            .unwrap();
        store
            .save_turn("session-order", 0, &turn0, None)
            .await
            .unwrap();
        store
            .save_turn("session-order", 1, &turn1, None)
            .await
            .unwrap();

        let loaded = store.load_turns("session-order").await.unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0], turn0, "Turn 0 should be first");
        assert_eq!(loaded[1], turn1, "Turn 1 should be second");
        assert_eq!(loaded[2], turn2, "Turn 2 should be third");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn load_full_history_flattens_per_turn_messages() {
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-fh", "/ws", "model", "provider")
            .await
            .unwrap();

        let turn0 = make_messages("Turn zero");
        let turn1 = make_messages("Turn one");

        store.save_turn("session-fh", 0, &turn0, None).await.unwrap();
        store.save_turn("session-fh", 1, &turn1, None).await.unwrap();

        let full = store.load_full_history("session-fh").await.unwrap();
        assert_eq!(full.len(), 2);
        assert_eq!(full[0], turn0[0]);
        assert_eq!(full[1], turn1[0]);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn truncate_turns_removes_turns_at_and_after_specified_index() {
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-tr", "/ws", "model", "provider")
            .await
            .unwrap();

        let turn0 = make_messages("Turn zero");
        let turn1 = make_messages("Turn one");
        let turn2 = make_messages("Turn two");

        store.save_turn("session-tr", 0, &turn0, None).await.unwrap();
        store.save_turn("session-tr", 1, &turn1, None).await.unwrap();
        store.save_turn("session-tr", 2, &turn2, None).await.unwrap();

        // Also save some todos and verify they are preserved!
        let todos = vec![TodoItem {
            id: "1".to_string(),
            content: "Task".to_string(),
            status: TodoStatus::Pending,
            priority: TodoPriority::Medium,
        }];
        store.save_todos("session-tr", &todos).await.unwrap();

        // Truncate turns starting from index 1 (removes turn 1 and turn 2)
        store.truncate_turns("session-tr", 1).await.unwrap();

        let loaded = store.load_turns("session-tr").await.unwrap();
        assert_eq!(loaded.len(), 1, "Only turn 0 should remain");
        assert_eq!(loaded[0], turn0);

        let loaded_todos = store.load_todos("session-tr").await.unwrap();
        assert_eq!(loaded_todos.len(), 1, "Todos must be preserved across turn truncation");
        assert_eq!(loaded_todos[0].content, "Task");

        let _ = std::fs::remove_file(path);
    }
}

