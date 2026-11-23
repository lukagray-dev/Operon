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
    /// Context compaction records preserving compacted snapshots for the LLM runner.
    #[serde(default)]
    pub compactions: Vec<CompactionRecord>,
    /// Session-scoped todo items created and managed during this session.
    /// Defaulted to empty vector for backward compatibility with older session files.
    #[serde(default)]
    pub todos: Vec<TodoItem>,
}

/// Represents a context compaction event persisted to disk.
///
/// Hey friend! When context compaction runs, older conversation turns are summarized
/// to keep the active token budget under control. To make sure the user never loses
/// their chat history in the UI, we preserve all original turns in `session.turns`,
/// while saving this compaction record so the SessionRunner can resume from the
/// compacted baseline without exceeding the model's context window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRecord {
    /// Turn index at which context compaction was performed.
    pub turn_index: usize,
    /// Unix timestamp when the compaction was performed.
    pub compacted_at: i64,
    /// Token count before compaction occurred.
    pub tokens_before: usize,
    /// Token count after compaction occurred.
    pub tokens_after: usize,
    /// Summarized text of the compacted turns.
    pub summary: String,
    /// Compacted message baseline (system snapshot, assistant summary, and preserved turns).
    pub compacted_baseline: Vec<ConversationMessage>,
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

        // Build the session structure with empty turns, compactions, and todos lists initially.
        let session = SessionJson {
            id: session_id.to_string(),
            created_at,
            workspace: workspace.to_string(),
            model_id: model_id.to_string(),
            provider: provider.to_string(),
            turns: Vec::new(),
            compactions: Vec::new(),
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
                compactions: Vec::new(),
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

    /// Record a context compaction event without destroying historical turns.
    ///
    /// When context compaction runs, older turns are summarized into a fresh system snapshot
    /// and summary message while preserving recent turns. This method records the compaction
    /// event with its baseline so the SessionRunner can resume from the compacted state, while
    /// keeping all existing `session.turns` intact so that users can view and scroll through
    /// the full conversation history in the GUI and other frontends.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read/write or JSON serialization failure.
    pub async fn apply_compaction(
        &self,
        session_id: &str,
        turn_index: usize,
        compacted_baseline: &[ConversationMessage],
        tokens_before: usize,
        tokens_after: usize,
        summary: &str,
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
                compactions: Vec::new(),
                todos: Vec::new(),
            }
        };

        let record = CompactionRecord {
            turn_index,
            compacted_at: unix_timestamp_secs() as i64,
            tokens_before,
            tokens_after,
            summary: summary.to_string(),
            compacted_baseline: compacted_baseline.to_vec(),
        };

        // If a compaction record for this turn already exists, replace it; otherwise append.
        if let Some(pos) = session
            .compactions
            .iter()
            .position(|c| c.turn_index == turn_index)
        {
            session.compactions[pos] = record;
        } else {
            session.compactions.push(record);
        }

        session.compactions.sort_by_key(|c| c.turn_index);

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
                compactions: Vec::new(),
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
    pub async fn load_todos(&self, _session_id: &str) -> Result<Vec<TodoItem>, SessionError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        Ok(session.todos)
    }

    /// Load all context compaction records associated with this session.
    ///
    /// Returns an empty vector if the session file does not exist or has no compactions.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or JSON deserialization failure.
    pub async fn load_compactions(
        &self,
        _session_id: &str,
    ) -> Result<Vec<CompactionRecord>, SessionError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        Ok(session.compactions)
    }

    /// Truncate all saved turns with turn_index >= keep_turns_count for a session.
    ///
    /// This removes all saved turns starting from `keep_turns_count` onwards,
    /// enabling editing a previous user turn and continuing the conversation from there.
    /// Note: session todos are preserved during truncation, and any compaction records
    /// at or beyond keep_turns_count are pruned cleanly!
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
        session.compactions.retain(|c| c.turn_index < keep_turns_count);

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

    /// Load all turns for a session with their turn_index and created_at timestamps in ascending order.
    ///
    /// Returns a `Vec<(usize, i64, Vec<ConversationMessage>)>` where elements are:
    /// `(turn_index, created_at, messages)`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or JSON deserialization failure.
    pub async fn load_turns_with_metadata(
        &self,
        _session_id: &str,
    ) -> Result<Vec<(usize, i64, Vec<ConversationMessage>)>, SessionError> {
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
            .map(|t| (t.turn_index, t.created_at, t.messages))
            .collect())
    }

    /// Load the active conversation history tailored for LLM SessionRunner execution.
    ///
    /// If context compaction has occurred, this returns the latest compaction baseline
    /// followed by all messages from turns that completed strictly AFTER that compaction.
    /// If no compaction has occurred, it returns all conversation messages across all turns.
    /// This ensures the LLM never blows past token limits when resuming a session, while
    /// full conversation history remains available in `load_turns_with_metadata` / `load_turns_with_timestamps`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or JSON deserialization failure.
    pub async fn load_runner_history(
        &self,
        _session_id: &str,
    ) -> Result<Vec<ConversationMessage>, SessionError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file_content = std::fs::read_to_string(&self.path)
            .map_err(|e| SessionError::Store(format!("Failed to read session file: {e}")))?;
        let mut session = serde_json::from_str::<SessionJson>(&file_content)
            .map_err(|e| SessionError::Store(format!("Failed to parse session file: {e}")))?;

        session.turns.sort_by_key(|t| t.turn_index);

        if let Some(latest_compaction) = session.compactions.into_iter().max_by_key(|c| c.turn_index) {
            let mut runner_history = latest_compaction.compacted_baseline;
            for turn in session.turns {
                // Hey friend! Turns strictly before `latest_compaction.turn_index` were summarized
                // into the compacted baseline. Turns occurring at or after `latest_compaction.turn_index`
                // completed during or after compaction, so their messages must be appended to resume
                // the conversation accurately!
                if turn.turn_index >= latest_compaction.turn_index {
                    runner_history.extend(turn.messages);
                }
            }
            Ok(runner_history)
        } else {
            Ok(session.turns.into_iter().flat_map(|t| t.messages).collect())
        }
    }

    /// Load all messages across all turns in chronological order for runner execution.
    ///
    /// Delegates to `load_runner_history`, ensuring compaction baselines are respected
    /// when resuming an agent session.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Store`] on file read or JSON deserialization failure.
    pub async fn load_full_history(
        &self,
        session_id: &str,
    ) -> Result<Vec<ConversationMessage>, SessionError> {
        self.load_runner_history(session_id).await
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

    /// Get the token count of the last recorded turn or compaction for a session.
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

        let last_turn = session.turns.iter().max_by_key(|t| t.turn_index);
        let latest_compaction = session.compactions.iter().max_by_key(|c| c.turn_index);

        match (last_turn, latest_compaction) {
            (Some(turn), Some(comp)) => {
                // Hey friend! If compaction happened on a later turn or occurred after this turn was saved,
                // compaction's `tokens_after` is the newer token measure.
                // Otherwise, the turn finished after compaction and its token count reflects the active state.
                if comp.turn_index > turn.turn_index
                    || (comp.turn_index == turn.turn_index && comp.compacted_at > turn.created_at)
                {
                    Ok(Some(comp.tokens_after))
                } else {
                    Ok(turn.token_count.or(Some(comp.tokens_after)))
                }
            }
            (Some(turn), None) => Ok(turn.token_count),
            (None, Some(comp)) => Ok(Some(comp.tokens_after)),
            (None, None) => Ok(None),
        }
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
        assert!(
            loaded.is_empty(),
            "Missing todos key should default to empty vector"
        );

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

        store
            .save_turn("session-fh", 0, &turn0, None)
            .await
            .unwrap();
        store
            .save_turn("session-fh", 1, &turn1, None)
            .await
            .unwrap();

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

        store
            .save_turn("session-tr", 0, &turn0, None)
            .await
            .unwrap();
        store
            .save_turn("session-tr", 1, &turn1, None)
            .await
            .unwrap();
        store
            .save_turn("session-tr", 2, &turn2, None)
            .await
            .unwrap();

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
        assert_eq!(
            loaded_todos.len(),
            1,
            "Todos must be preserved across turn truncation"
        );
        assert_eq!(loaded_todos[0].content, "Task");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn apply_compaction_preserves_historical_turns() {
        // Hey buddy! This test verifies Issue 1's fix:
        // When context compaction is triggered (e.g. after Turn 1), the session store must
        // NOT wipe existing historical turns (Turn 0 and Turn 1). They must stay safely on disk
        // so the user can scroll back through the full conversation in the GUI!
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-compact-hist", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let turn0 = make_messages("Turn zero content");
        let turn1 = make_messages("Turn one content");

        store
            .save_turn("session-compact-hist", 0, &turn0, Some(500))
            .await
            .expect("save turn 0");
        store
            .save_turn("session-compact-hist", 1, &turn1, Some(1000))
            .await
            .expect("save turn 1");

        // Now simulate compaction occurring at turn_index 1
        let compacted_baseline = make_messages("Summary of turns 0 and 1");
        store
            .apply_compaction(
                "session-compact-hist",
                1,
                &compacted_baseline,
                1000,
                200,
                "Summary of turns 0 and 1",
            )
            .await
            .expect("apply_compaction");

        // Verify that all historical turns are still completely preserved!
        let loaded_turns = store
            .load_turns("session-compact-hist")
            .await
            .expect("load_turns");
        assert_eq!(
            loaded_turns.len(),
            2,
            "Both Turn 0 and Turn 1 must still exist after compaction"
        );
        assert_eq!(loaded_turns[0], turn0);
        assert_eq!(loaded_turns[1], turn1);

        // Verify that load_turns_with_metadata keeps the real turn indices
        let loaded_meta = store
            .load_turns_with_metadata("session-compact-hist")
            .await
            .expect("load_turns_with_metadata");
        assert_eq!(loaded_meta.len(), 2);
        assert_eq!(loaded_meta[0].0, 0, "Turn 0 index must be 0");
        assert_eq!(loaded_meta[1].0, 1, "Turn 1 index must be 1");

        // Verify that the compaction record was saved properly
        let compactions = store
            .load_compactions("session-compact-hist")
            .await
            .expect("load_compactions");
        assert_eq!(compactions.len(), 1);
        assert_eq!(compactions[0].turn_index, 1);
        assert_eq!(compactions[0].tokens_before, 1000);
        assert_eq!(compactions[0].tokens_after, 200);
        assert_eq!(compactions[0].summary, "Summary of turns 0 and 1");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn load_runner_history_resumes_from_compaction_baseline() {
        // Hey buddy! This test verifies that while all turns are preserved for the GUI display,
        // when the LLM runner resumes the session, it loads ONLY the compacted baseline plus turns
        // occurring at or after compaction. That way, older summarized turns are excluded,
        // while the active and subsequent turns are preserved for seamless continuation!
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-runner-hist", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let turn0 = make_messages("Turn 0 user prompt and assistant answer");
        let turn1 = make_messages("Turn 1 user prompt and assistant answer");

        store
            .save_turn("session-runner-hist", 0, &turn0, Some(500))
            .await
            .unwrap();

        let baseline = make_messages("Compacted summary of turn 0");
        store
            .apply_compaction(
                "session-runner-hist",
                1,
                &baseline,
                1000,
                150,
                "Compacted summary of turn 0",
            )
            .await
            .unwrap();

        // Right after compaction at turn 1, before turn 1 completes, runner history is ONLY the baseline
        let runner_hist1 = store
            .load_runner_history("session-runner-hist")
            .await
            .unwrap();
        assert_eq!(
            runner_hist1, baseline,
            "Runner history should only be the compacted baseline before turn 1 messages are saved"
        );

        // Turn 1 completes and saves its messages
        store
            .save_turn("session-runner-hist", 1, &turn1, Some(1000))
            .await
            .unwrap();

        // Now user sends a new message after compaction (Turn 2)
        let turn2 = make_messages("Turn 2 post-compaction prompt");
        store
            .save_turn("session-runner-hist", 2, &turn2, Some(300))
            .await
            .unwrap();

        // Runner history should now be [baseline + Turn 1 + Turn 2]
        let runner_hist2 = store
            .load_runner_history("session-runner-hist")
            .await
            .unwrap();
        assert_eq!(
            runner_hist2.len(),
            baseline.len() + turn1.len() + turn2.len(),
            "Runner history must combine the baseline and turns at or after compaction"
        );
        assert_eq!(runner_hist2[0], baseline[0]);
        assert_eq!(runner_hist2[1], turn1[0]);
        assert_eq!(runner_hist2[2], turn2[0]);

        // Full UI history should still have all 3 turns!
        let all_turns = store.load_turns("session-runner-hist").await.unwrap();
        assert_eq!(all_turns.len(), 3, "All 3 turns must remain in UI history");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn truncate_turns_removes_compactions_after_cutoff() {
        // Hey buddy! If a user rewinds or edits a turn that occurred before a compaction,
        // any compactions that happened at or after the cutoff turn must be removed
        // so the session state stays consistent!
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-trunc-comp", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let turn0 = make_messages("Turn 0");
        let turn1 = make_messages("Turn 1");
        let turn2 = make_messages("Turn 2");

        store
            .save_turn("session-trunc-comp", 0, &turn0, None)
            .await
            .unwrap();
        store
            .save_turn("session-trunc-comp", 1, &turn1, None)
            .await
            .unwrap();

        // Compact at turn 1
        let baseline = make_messages("Baseline at turn 1");
        store
            .apply_compaction("session-trunc-comp", 1, &baseline, 800, 200, "Summary")
            .await
            .unwrap();

        // Add turn 2
        store
            .save_turn("session-trunc-comp", 2, &turn2, None)
            .await
            .unwrap();

        assert_eq!(
            store.load_compactions("session-trunc-comp").await.unwrap().len(),
            1
        );

        // Truncate starting at turn 1 (e.g. user edits prompt 1).
        // This removes turn 1, turn 2, and the compaction at turn 1!
        store.truncate_turns("session-trunc-comp", 1).await.unwrap();

        let compactions = store.load_compactions("session-trunc-comp").await.unwrap();
        assert!(
            compactions.is_empty(),
            "Compaction at turn 1 must be pruned when truncating from turn 1"
        );

        let turns = store.load_turns("session-trunc-comp").await.unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0], turn0);

        // Runner history should now fallback to turn 0
        let runner_hist = store
            .load_runner_history("session-trunc-comp")
            .await
            .unwrap();
        assert_eq!(runner_hist, turn0);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_compaction_baseline_preserves_in_flight_turn_user_prompt() {
        // Hey buddy! This test simulates a real mid-turn compaction where:
        // 1. Turn 0 completes normally.
        // 2. Turn 1 starts with a User prompt and makes tool calls.
        // 3. Compaction triggers mid-turn: `compacted_baseline` stores only messages before the in-flight user prompt.
        // 4. Turn 1 finishes and saves its messages.
        // 5. We verify that Turn 1's saved messages start with the User prompt (not a Tool or Assistant message),
        //    preventing UI ordering glitches and missing user prompts!
        let path = temp_store_path().await;
        let store = SessionStore::open(&path)
            .await
            .expect("Failed to open store");

        store
            .create_session("session-midturn-comp", "/ws", "model", "provider")
            .await
            .expect("create_session");

        let turn0 = make_messages("Turn 0 user and assistant");
        store
            .save_turn("session-midturn-comp", 0, &turn0, Some(500))
            .await
            .unwrap();

        // Simulate messages array inside runner when compaction triggers mid-turn on Turn 1:
        // [System snapshot, Assistant summary, User prompt (turn 1), Tool result]
        let system_msg = operon_context::ConversationMessage::system("System Snapshot");
        let summary_msg = operon_context::ConversationMessage::assistant(vec![
            operon_context::ContentBlock::Text("— Previous session summary —\n\nTurn 0 summarized".to_string()),
        ]);
        let user1_msg = operon_context::ConversationMessage::user(vec![
            operon_context::ContentBlock::Text("Turn 1 user request".to_string()),
        ]);
        let tool_msg = operon_context::ConversationMessage {
            role: operon_context::MessageRole::Tool,
            content: vec![],
            stop_reason: None,
        };

        let messages_after_compaction = vec![
            system_msg.clone(),
            summary_msg.clone(),
            user1_msg.clone(),
            tool_msg.clone(),
        ];

        // 1. Slicing compacted_baseline before the in-flight user message:
        let in_flight_user_idx = messages_after_compaction
            .iter()
            .rposition(|m| m.role == operon_context::MessageRole::User)
            .unwrap();
        assert_eq!(in_flight_user_idx, 2);

        let compacted_baseline = &messages_after_compaction[..in_flight_user_idx];
        assert_eq!(compacted_baseline.len(), 2);
        assert_eq!(compacted_baseline[0].role, operon_context::MessageRole::System);
        assert_eq!(compacted_baseline[1].role, operon_context::MessageRole::Assistant);

        store
            .apply_compaction(
                "session-midturn-comp",
                1,
                compacted_baseline,
                140_000,
                15_000,
                "Turn 0 summarized",
            )
            .await
            .unwrap();

        // 2. Turn 1 concludes with an assistant message:
        let assistant1_msg = operon_context::ConversationMessage::assistant(vec![
            operon_context::ContentBlock::Text("Turn 1 final answer".to_string()),
        ]);
        let mut final_messages = messages_after_compaction.clone();
        final_messages.push(assistant1_msg.clone());

        // In loop_impl.rs, turn_start_len is reset to rposition(|m| m.role == MessageRole::User):
        let turn_start_len = final_messages
            .iter()
            .rposition(|m| m.role == operon_context::MessageRole::User)
            .unwrap();
        assert_eq!(turn_start_len, 2);

        let turn1_saved_messages = &final_messages[turn_start_len..];
        assert_eq!(turn1_saved_messages[0].role, operon_context::MessageRole::User);
        assert_eq!(turn1_saved_messages.len(), 3); // User, Tool, Assistant

        store
            .save_turn("session-midturn-comp", 1, turn1_saved_messages, Some(16_000))
            .await
            .unwrap();

        // 3. Verify turns loaded for UI via load_turns_with_metadata:
        let turns_with_meta = store
            .load_turns_with_metadata("session-midturn-comp")
            .await
            .unwrap();
        assert_eq!(turns_with_meta.len(), 2);
        assert_eq!(turns_with_meta[1].0, 1, "Turn 1 index");
        assert_eq!(
            turns_with_meta[1].2[0].role,
            operon_context::MessageRole::User,
            "Turn 1 must start with the User message so GUI renders prompt first!"
        );

        // 4. Verify runner history resumed:
        let runner_history = store
            .load_runner_history("session-midturn-comp")
            .await
            .unwrap();
        // baseline (2) + turn 1 (3) = 5 messages total
        assert_eq!(runner_history.len(), 5);
        assert_eq!(runner_history[0].role, operon_context::MessageRole::System);
        assert_eq!(runner_history[1].role, operon_context::MessageRole::Assistant);
        assert_eq!(runner_history[2].role, operon_context::MessageRole::User);

        let _ = std::fs::remove_file(path);
    }
}
