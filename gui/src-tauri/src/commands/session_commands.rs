// session_commands.rs — Tauri IPC command handlers for agent sessions.
//
// Hey friend! This module provides the connection between the Tauri GUI and the `operon-rs`
// session runner, JSON store, and event systems.
//
// Features:
//   - List saved historical sessions by scanning `~/.operon/sessions/*.json`.
//   - Retrieve message turns for a given session.
//   - Start/resume a session, forwarding all `SessionEvent`s to the webview.
//   - Send Approve/Deny/Cancel signals back to the running agent loop.

use crate::commands::model_commands::SharedState;
use operon_rs::{
    config::OperonPaths,
    events::{SessionCommand, SessionEvent},
    prelude::Role,
    session::{store::SessionStore, SessionConfig, SessionRunner},
};
use std::path::PathBuf;
use tauri::{Emitter, State, WebviewWindow};
use tokio::sync::mpsc;

// Hey friend! std::fs::canonicalize() on Windows prepends the \\?\ UNC prefix.
// We define a helper to strip it so paths displayed in the GUI are clean and standard.
fn clean_unc_path(s: String) -> String {
    if s.starts_with(r"\\?\") {
        s[4..].to_string()
    } else {
        s
    }
}

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
    /// True when this session's workspace is a project directory
    /// (i.e. not ~/.operon/workspace/).
    pub is_project: bool,
    /// The folder name of the project directory (e.g. "Operon").
    /// Empty string when is_project is false.
    pub project_name: String,
}

/// Retrieve the list of all historical sessions saved on the system.
///
/// This works by scanning the standard sessions directory (~/.operon/sessions/)
/// for `.json` files, opening each one to read the session metadata, and returning
/// them sorted from newest to oldest.
#[tauri::command]
pub async fn list_sessions() -> Result<Vec<SessionItem>, String> {
    // Resolve platform-specific ~/.operon paths
    let paths = OperonPaths::resolve().map_err(|e| e.to_string())?;
    let sessions_dir = paths.sessions_dir;

    let default_workspace = {
        let p = paths
            .workspace_dir
            .canonicalize()
            .unwrap_or_else(|_| paths.workspace_dir.clone())
            .to_string_lossy()
            .to_string();
        clean_unc_path(p)
    };

    let mut sessions = Vec::new();
    if sessions_dir.exists() {
        let entries = std::fs::read_dir(sessions_dir).map_err(|e| e.to_string())?;
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                // Hey buddy! We now look for files with a `.json` extension since
                // we migrated away from SQLite databases.
                if path.extension().map_or(false, |ext| ext == "json") {
                    // Open the JSON store to extract session metadata
                    if let Ok(store) = SessionStore::open(&path).await {
                        if let Ok(rows) = store.list_sessions().await {
                            // Since each JSON file represents a single session, we take the first row
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

                                let session_workspace_canon = {
                                    let p = std::path::PathBuf::from(&row.workspace)
                                        .canonicalize()
                                        .unwrap_or_else(|_| {
                                            std::path::PathBuf::from(&row.workspace)
                                        })
                                        .to_string_lossy()
                                        .to_string();
                                    clean_unc_path(p)
                                };

                                let is_project = session_workspace_canon != default_workspace;
                                let project_name = if is_project {
                                    std::path::Path::new(&row.workspace)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("")
                                        .to_string()
                                } else {
                                    String::new()
                                };

                                sessions.push(SessionItem {
                                    id: row.id.clone(),
                                    created_at: row.created_at,
                                    workspace: row.workspace.clone(),
                                    model_id: row.model_id.clone(),
                                    provider: row.provider.clone(),
                                    title,
                                    is_project,
                                    project_name,
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
/// loads all turns from the JSON file and returns the final turn's complete
/// messages list (which contains the full conversation history up to that point).
#[tauri::command]
pub async fn get_session_history(
    session_id: String,
) -> Result<Vec<operon_rs::prelude::ConversationMessage>, String> {
    let paths = OperonPaths::resolve().map_err(|e| e.to_string())?;
    let mut json_path = paths.session_db(&session_id);

    // Fallback: If the session file (<session_id>.json) does not exist,
    // search the sessions directory to see if any json file contains
    // this session_id inside its records (due to previous mismatched session ID bugs).
    if !json_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&paths.sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    if let Ok(store) = SessionStore::open(&path).await {
                        if let Ok(rows) = store.list_sessions().await {
                            if rows.iter().any(|r| r.id == session_id) {
                                json_path = path;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if !json_path.exists() {
        return Ok(Vec::new());
    }

    let store = SessionStore::open(&json_path)
        .await
        .map_err(|e| e.to_string())?;

    let turns = store
        .load_turns(&session_id)
        .await
        .map_err(|e| e.to_string())?;

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

    if app_config.provider.credentials.api_key.is_empty()
        && app_config.provider.provider != operon_rs::prelude::Provider::Ollama
    {
        return Err("API key is missing. Please configure a provider in settings.".to_string());
    }

    // Lock global AppState to manage active runner sessions
    let mut state_guard = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;

    if state_guard.active_sessions.contains_key(&session_id) {
        return Err("This session is already running a request.".to_string());
    }

    // Create the command channel for other endpoints to interact with this runner (Approve/Deny/Cancel)
    let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(16);
    state_guard
        .active_sessions
        .insert(session_id.clone(), cmd_tx);
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

            let mut json_path = paths.session_db(&session_id_clone);

            // Fallback: If the session JSON file (<session_id>.json) does not exist,
            // search the sessions directory to see if any JSON file contains
            // this session_id inside its records (due to previous mismatched session ID bugs).
            if !json_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&paths.sessions_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |ext| ext == "json") {
                            if let Ok(store) = SessionStore::open(&path).await {
                                if let Ok(rows) = store.list_sessions().await {
                                    if rows.iter().any(|r| r.id == session_id_clone) {
                                        json_path = path;
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
                tool_groups: vec![
                    "fs".into(),
                    "shell".into(),
                    "web".into(),
                    "todo".into(),
                    "ask".into(),
                    "memory".into(),
                ],
                compaction: operon_rs::prelude::CompactionConfig::default(),
                store_path: Some(json_path.clone()),
            };

            let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(256);

            // Initialize runner
            let mut runner = SessionRunner::new(session_config, event_tx, cmd_rx)
                .await
                .map_err(|e| format!("Failed to initialize SessionRunner: {}", e))?;

            // Resume history from JSON file if resuming an existing session
            if json_path.exists() {
                if let Ok(store) = SessionStore::open(&json_path).await {
                    let turns = store
                        .load_turns(&session_id_clone)
                        .await
                        .unwrap_or_default();
                    if !turns.is_empty() {
                        let history = turns.last().cloned().unwrap_or_default();
                        let turn_index = turns.len();
                        let last_token_count = store
                            .get_last_token_count(&session_id_clone)
                            .await
                            .unwrap_or_default();
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
        }
        .await;

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
pub async fn cancel_session(
    session_id: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let tx = {
        let state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_sessions.get(&session_id).cloned()
    };
    if let Some(tx) = tx {
        tx.send(SessionCommand::Cancel)
            .await
            .map_err(|e| format!("Failed to send cancel: {}", e))?;
        Ok(())
    } else {
        Err("Session is not active or running".to_string())
    }
}

/// Send an Approve command to the running session's pending approval tool call.
#[tauri::command]
pub async fn approve_tool_call(
    session_id: String,
    id: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let tx = {
        let state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_sessions.get(&session_id).cloned()
    };
    if let Some(tx) = tx {
        tx.send(SessionCommand::Approve { id })
            .await
            .map_err(|e| format!("Failed to send approve: {}", e))?;
        Ok(())
    } else {
        Err("Session is not active or running".to_string())
    }
}

/// Send a Deny command to the running session's pending approval tool call.
#[tauri::command]
pub async fn deny_tool_call(
    session_id: String,
    id: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let tx = {
        let state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_sessions.get(&session_id).cloned()
    };
    if let Some(tx) = tx {
        tx.send(SessionCommand::Deny { id })
            .await
            .map_err(|e| format!("Failed to send deny: {}", e))?;
        Ok(())
    } else {
        Err("Session is not active or running".to_string())
    }
}

/// Send the user's answer to a suspended `ask` tool call.
///
/// Hey friend! This is called by the UI when the user picks one of the 3 MCQ
/// options or submits their own free-text answer. The `id` must match the
/// `AskQuestion` event's `id` field that the frontend received earlier.
#[tauri::command]
pub async fn answer_ask(
    session_id: String,
    id: String,
    answer: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let tx = {
        let state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_sessions.get(&session_id).cloned()
    };
    if let Some(tx) = tx {
        tx.send(SessionCommand::AskResponse { id, answer })
            .await
            .map_err(|e| format!("Failed to send ask response: {}", e))?;
        Ok(())
    } else {
        Err("Session is not active or running".to_string())
    }
}

/// Open a native folder picker and register the selected folder as an allowed
/// directory in config.toml (if not already present).
///
/// Hey friend! This is the entry point for "Files → Open Folder". The picked
/// directory is added to config.toml with ask/ask defaults so the user can
/// configure its permissions from the Permissions settings panel. If the folder
/// is already in config.toml, we skip the write — existing permissions are
/// untouched.
///
/// Returns the selected path, or None if the user cancelled the picker.
#[tauri::command]
pub async fn open_project_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    // Show the native OS folder picker and block until the user picks or cancels.
    let folder = app.dialog().file().blocking_pick_folder();

    let path = match folder {
        Some(p) => p
            .into_path()
            .map_err(|e| format!("Invalid folder path: {}", e))?
            .to_string_lossy()
            .to_string(),
        None => return Ok(None), // User cancelled — not an error
    };

    // Add to config.toml if not already present. add_allowed_directory() is
    // idempotent — it checks for duplicates before writing.
    operon_rs::config::add_allowed_directory(&path)
        .map_err(|e| format!("Failed to register project directory: {}", e))?;

    Ok(Some(path))
}

/// Return the canonical path of the default workspace directory (~/.operon/workspace/).
///
/// Hey buddy! The frontend uses this to classify sessions: sessions whose stored
/// workspace path matches the default workspace are shown under "Chats"; sessions
/// whose stored workspace path differs are grouped under "Projects" in the sidebar.
#[tauri::command]
pub async fn get_default_workspace() -> Result<String, String> {
    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    Ok(paths.workspace_dir.to_string_lossy().to_string())
}

/// Delete a specific chat session by deleting its JSON file and terminating its runner if active.
///
/// Hey friend! This command resolves the session's JSON storage file path and deletes it from
/// the ~/.operon/sessions directory. If the session is currently active/running, we send a cancel
/// signal to the running agent thread to terminate it safely.
/// We show a native confirmation dialog before deletion.
#[tauri::command]
pub async fn delete_session(
    app: tauri::AppHandle,
    session_id: String,
    state: State<'_, SharedState>,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    // Hey buddy! We show a native message confirmation dialog.
    let confirmed = app
        .dialog()
        .message("Are you sure you want to delete this chat session?")
        .title("Delete Chat Session")
        .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancel)
        .blocking_show();

    if !confirmed {
        return Ok(false);
    }

    let paths = OperonPaths::resolve().map_err(|e| e.to_string())?;
    let json_path = paths.session_db(&session_id);

    // If the session is currently active/running, cancel it first
    let tx = {
        let state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_sessions.get(&session_id).cloned()
    };
    if let Some(tx) = tx {
        let _ = tx.send(SessionCommand::Cancel).await;
        // Remove it from active sessions list
        if let Ok(mut lock) = state.lock() {
            lock.active_sessions.remove(&session_id);
        }
    }

    if json_path.exists() {
        std::fs::remove_file(json_path)
            .map_err(|e| format!("Failed to delete session file: {}", e))?;
    }
    Ok(true)
}

/// Delete a project and all its associated chat sessions.
///
/// Hey friend! This command scans the sessions folder, reads every session file, and deletes any
/// file whose workspace matches the target project path. It also terminates any active runners
/// running for those sessions, and finally removes the project from the allowed list in config.toml.
/// We show a native confirmation dialog before deletion.
#[tauri::command]
pub async fn delete_project(
    app: tauri::AppHandle,
    project_path: String,
    state: State<'_, SharedState>,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    let project_name = std::path::Path::new(&project_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project");

    // Hey buddy! We show a native message confirmation dialog.
    let confirmed = app.dialog()
        .message(format!(
            "Are you sure you want to delete project \"{}\"? This will delete all its chat sessions and remove it from the sidebar.",
            project_name
        ))
        .title("Delete Project")
        .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancel)
        .blocking_show();

    if !confirmed {
        return Ok(false);
    }

    let paths = OperonPaths::resolve().map_err(|e| e.to_string())?;
    let sessions_dir = paths.sessions_dir;

    let target_canon = {
        let p = std::path::PathBuf::from(&project_path)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(&project_path))
            .to_string_lossy()
            .to_string();
        clean_unc_path(p)
    };

    if sessions_dir.exists() {
        let entries = std::fs::read_dir(sessions_dir).map_err(|e| e.to_string())?;
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    if let Ok(store) = SessionStore::open(&path).await {
                        if let Ok(rows) = store.list_sessions().await {
                            if let Some(row) = rows.first() {
                                let row_canon = {
                                    let p = std::path::PathBuf::from(&row.workspace)
                                        .canonicalize()
                                        .unwrap_or_else(|_| std::path::PathBuf::from(&row.workspace))
                                        .to_string_lossy()
                                        .to_string();
                                    clean_unc_path(p)
                                };

                                if row_canon == target_canon {
                                    // Cancel if active
                                    let tx = {
                                        let state_guard = state
                                            .lock()
                                            .map_err(|e| format!("Failed to lock state: {}", e))?;
                                        state_guard.active_sessions.get(&row.id).cloned()
                                    };
                                    if let Some(tx) = tx {
                                        let _ = tx.send(SessionCommand::Cancel).await;
                                        if let Ok(mut lock) = state.lock() {
                                            lock.active_sessions.remove(&row.id);
                                        }
                                    }
                                    // Delete the session JSON file
                                    let _ = std::fs::remove_file(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Remove the project path from allowed directories in config.toml
    let _ = operon_rs::config::remove_allowed_directory(&project_path);

    Ok(true)
}
