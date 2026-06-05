// session_commands.rs — Tauri IPC command handlers for agent sessions.
//
// This module provides the connection between the Tauri GUI and the `operon-rs`
// session runner, SQLite store, and event systems.
//
// Features:
//   - List saved historical sessions by scanning `~/.operon/sessions/*.db`.
//   - Retrieve message turns for a given session.
//   - Start/resume a session, forwarding all `SessionEvent`s to the webview.
//   - Send Approve/Deny/Cancel signals back to the running agent loop.

use std::path::PathBuf;
use tokio::sync::mpsc;
use tauri::{State, WebviewWindow, Emitter};
use operon_rs::{
    config::OperonPaths,
    events::{SessionCommand, SessionEvent},
    session::{SessionConfig, SessionRunner, store::SessionStore},
    prelude::Role,
};
use crate::commands::model_commands::SharedState;

/// Data Transfer Object (DTO) for listing sessions in the sidebar.
/// This matches the shape expected by the frontend.
#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionItem {
    pub id: String,
    pub created_at: i64,
    pub workspace: String,
    pub model_id: String,
    pub provider: String,
    pub title: String,
}

/// Retrieve the list of all historical sessions saved on the system.
///
/// This works by scanning the standard sessions directory (~/.operon/sessions/)
/// for `.db` files, opening each one to read the session metadata, and returning
/// them sorted from newest to oldest.
#[tauri::command]
pub async fn list_sessions() -> Result<Vec<SessionItem>, String> {
    // Resolve platform-specific ~/.operon paths
    let paths = OperonPaths::resolve().map_err(|e| e.to_string())?;
    let sessions_dir = paths.sessions_dir;

    let mut sessions = Vec::new();
    if sessions_dir.exists() {
        let entries = std::fs::read_dir(sessions_dir).map_err(|e| e.to_string())?;
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                // We only check files that have a `.db` extension
                if path.extension().map_or(false, |ext| ext == "db") {
                    // Open SQLite store in WAL read-only mode to extract metadata
                    if let Ok(store) = SessionStore::open(&path).await {
                        if let Ok(rows) = store.list_sessions().await {
                            // Since each DB represents a single session, we take the first row
                            if let Some(row) = rows.first() {
                                let title = match store.get_first_user_message_text(&row.id).await {
                                    Ok(Some(text)) => {
                                        let clean_text = text.replace('\n', " ").trim().to_string();
                                        if clean_text.len() > 40 {
                                            format!("{}...", &clean_text[..40])
                                        } else {
                                            clean_text
                                        }
                                    }
                                    _ => "Untitled Chat".to_string(),
                                };
                                sessions.push(SessionItem {
                                    id: row.id.clone(),
                                    created_at: row.created_at,
                                    workspace: row.workspace.clone(),
                                    model_id: row.model_id.clone(),
                                    provider: row.provider.clone(),
                                    title,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort descending by created_at (most recent first)
    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(sessions)
}

/// Retrieve the complete conversation history for a given session.
///
/// If the session file does not exist, returns an empty list. Otherwise,
/// loads all turns from the SQLite DB and returns the final turn's complete
/// messages list (which contains the full conversation history up to that point).
#[tauri::command]
pub async fn get_session_history(session_id: String) -> Result<Vec<operon_rs::prelude::ConversationMessage>, String> {
    let paths = OperonPaths::resolve().map_err(|e| e.to_string())?;
    let mut db_path = paths.session_db(&session_id);

    // Fallback: If the session database file (<session_id>.db) does not exist,
    // search the sessions directory to see if any database file contains
    // this session_id inside its database records (due to previous mismatched session ID bugs).
    if !db_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&paths.sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "db") {
                    if let Ok(store) = SessionStore::open(&path).await {
                        if let Ok(rows) = store.list_sessions().await {
                            if rows.iter().any(|r| r.id == session_id) {
                                db_path = path;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let store = SessionStore::open(&db_path)
        .await
        .map_err(|e| e.to_string())?;

    let turns = store.load_turns(&session_id).await.map_err(|e| e.to_string())?;

    if turns.is_empty() {
        return Ok(Vec::new());
    }

    // The last turn contains the full accumulated history of all messages
    let full_history = turns.last().cloned().unwrap_or_default();
    Ok(full_history)
}

/// Start or resume an agent session runner and send a user message.
///
/// This command:
///   1. Loads the system's AppConfig (verifying that credentials/provider exist).
///   2. Checks if there is already an active runner task for this session ID.
///   3. Spawns a background task running the `SessionRunner` loop.
///   4. Loads database history if we are resuming an existing session.
///   5. Registers a forwarder to pipe all `SessionEvent`s as Tauri events.
///   6. Feeds the user message to the runner.
#[tauri::command]
pub async fn send_message(
    session_id: String,
    message: String,
    project_dir: Option<String>,
    state: State<'_, SharedState>,
    window: WebviewWindow,
) -> Result<(), String> {
    let paths = OperonPaths::resolve().map_err(|e| format!("Failed to resolve paths: {}", e))?;

    // Load active config and check credentials
    let app_config = operon_rs::load().map_err(|e| {
        format!("Failed to load configuration. Please configure your provider/credentials in settings: {}", e)
    })?;

    if app_config.provider.credentials.api_key.is_empty() && app_config.provider.provider != operon_rs::prelude::Provider::Ollama {
        return Err("API key is missing. Please configure a provider in settings.".to_string());
    }

    // Lock global AppState to manage active runner sessions
    let mut state_guard = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;

    if state_guard.active_sessions.contains_key(&session_id) {
        return Err("This session is already running a request.".to_string());
    }

    // Create the command channel for other endpoints to interact with this runner (Approve/Deny/Cancel)
    let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(16);
    state_guard.active_sessions.insert(session_id.clone(), cmd_tx);
    drop(state_guard); // Release state lock immediately

    let session_id_clone = session_id.clone();
    let state_clone = state.inner().clone();

    // Spawn a background tokio thread to execute the agentic loop
    tokio::spawn(async move {
        let run_result = async {
            let project_path = project_dir.map(PathBuf::from);
            // Decide snapshot and workspace root (PROJECT mode vs NORMAL mode)
            let workspace_root = if let Some(ref proj) = project_path {
                proj.clone()
            } else {
                paths.workspace_dir.clone()
            };

            let mut db_path = paths.session_db(&session_id_clone);

            // Fallback: If the session database file (<session_id>.db) does not exist,
            // search the sessions directory to see if any database file contains
            // this session_id inside its database records (due to previous mismatched session ID bugs).
            if !db_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&paths.sessions_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |ext| ext == "db") {
                            if let Ok(store) = SessionStore::open(&path).await {
                                if let Ok(rows) = store.list_sessions().await {
                                    if rows.iter().any(|r| r.id == session_id_clone) {
                                        db_path = path;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Construct standard session config
            let session_config = SessionConfig {
                provider_config: app_config.provider.clone(),
                policy: app_config.policy.clone(),
                project_dir: project_path,
                workspace_root,
                role: Role::Owner,
                // Default registers all standard tools
                tool_groups: vec!["fs".into(), "shell".into(), "web".into(), "todo".into()],
                compaction: operon_rs::prelude::CompactionConfig::default(),
                store_path: Some(db_path.clone()),
            };

            let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(256);

            // Initialize runner
            let mut runner = SessionRunner::new(session_config, event_tx, cmd_rx)
                .await
                .map_err(|e| format!("Failed to initialize SessionRunner: {}", e))?;

            // Resume history from SQLite database if resuming an existing session
            if db_path.exists() {
                if let Ok(store) = SessionStore::open(&db_path).await {
                    let turns = store.load_turns(&session_id_clone).await.unwrap_or_default();
                    if !turns.is_empty() {
                        let history = turns.last().cloned().unwrap_or_default();
                        let turn_index = turns.len();
                        let last_token_count = store.get_last_token_count(&session_id_clone).await.unwrap_or_default();
                        runner.set_history(history, turn_index, last_token_count);
                    }
                }
            }

            // Spawn forwarding task to emit events to Javascript webview window
            let window_clone = window.clone();
            let forward_task = tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let _ = window_clone.emit("session-event", event);
                }
            });

            // Run agent turn loop
            let run_outcome = runner.run(message).await;

            // Drop the runner explicitly to close the event sender channel.
            // This allows the event forwarding task to finish and resolves the deadlock/hang.
            drop(runner);

            // Wait for all events to be processed and sent
            let _ = forward_task.await;

            run_outcome.map_err(|e| format!("Agent execution error: {}", e))
        }.await;

        if let Err(err_msg) = run_result {
            // Forward runtime failures as session error events
            let _ = window.emit("session-event", SessionEvent::Error { message: err_msg });
        }

        // Clean up from active session list on completion/failure
        if let Ok(mut lock) = state_clone.lock() {
            lock.active_sessions.remove(&session_id_clone);
        }
    });

    Ok(())
}

/// Send a graceful Cancel command to the running session.
#[tauri::command]
pub async fn cancel_session(session_id: String, state: State<'_, SharedState>) -> Result<(), String> {
    let tx = {
        let state_guard = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_sessions.get(&session_id).cloned()
    };
    if let Some(tx) = tx {
        tx.send(SessionCommand::Cancel).await.map_err(|e| format!("Failed to send cancel: {}", e))?;
        Ok(())
    } else {
        Err("Session is not active or running".to_string())
    }
}

/// Send an Approve command to the running session's pending approval tool call.
#[tauri::command]
pub async fn approve_tool_call(session_id: String, id: String, state: State<'_, SharedState>) -> Result<(), String> {
    let tx = {
        let state_guard = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_sessions.get(&session_id).cloned()
    };
    if let Some(tx) = tx {
        tx.send(SessionCommand::Approve { id }).await.map_err(|e| format!("Failed to send approve: {}", e))?;
        Ok(())
    } else {
        Err("Session is not active or running".to_string())
    }
}

/// Send a Deny command to the running session's pending approval tool call.
#[tauri::command]
pub async fn deny_tool_call(session_id: String, id: String, state: State<'_, SharedState>) -> Result<(), String> {
    let tx = {
        let state_guard = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_sessions.get(&session_id).cloned()
    };
    if let Some(tx) = tx {
        tx.send(SessionCommand::Deny { id }).await.map_err(|e| format!("Failed to send deny: {}", e))?;
        Ok(())
    } else {
        Err("Session is not active or running".to_string())
    }
}
