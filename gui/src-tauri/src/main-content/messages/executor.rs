//! Agent Prompt Execution and Background SessionRunner Management.

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

use crate::main_content::input::PendingAttachmentDto;

/// Global thread-safe reference to the active session command channel for cancellation and approval.
pub static ACTIVE_CMD_TX: Mutex<Option<tokio::sync::mpsc::Sender<operon_rs::SessionCommand>>> =
    Mutex::new(None);

/// Submits a user prompt to the agent SessionRunner.
#[tauri::command]
pub async fn submit_prompt(
    app_handle: AppHandle,
    session_id: Option<String>,
    prompt: String,
    attachments: Vec<PendingAttachmentDto>,
    workspace_path: Option<String>,
) -> Result<String, String> {
    if prompt.trim().is_empty() && attachments.is_empty() {
        return Err("Prompt content cannot be empty".to_string());
    }

    let active_id = session_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            format!("session-{}", ts)
        });

    let app_config = operon_rs::load().map_err(|e| e.to_string())?;

    // Resolve workspace root from the frontend's current context:
    //   - PROJECT mode: workspace_path = Some("D:\MyProject") → use that path
    //   - GENERAL mode: workspace_path = None → use default workspace (~/.operon/workspace/)
    //
    // We intentionally do NOT read the historical workspace from the session store here.
    // The frontend knows the user's CURRENT context (which sidebar section the session
    // is under). If a session was moved from a project to general chats (or vice versa),
    // the frontend sends the new context, and we must respect that — not the old one.
    let workspace_root = match workspace_path {
        Some(ref p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => app_config.paths.workspace_dir.clone(),
    };

    // Ensure the workspace root directory physically exists on disk
    let _ = std::fs::create_dir_all(&workspace_root);

    // Set the process CWD to workspace_root so relative paths in tools
    // (e.g. `ls .`, shell commands) resolve against the active workspace
    let _ = std::env::set_current_dir(&workspace_root);

    let store_path = app_config.paths.session_db(&active_id);
    let is_new_session = !store_path.exists();
    let store = operon_rs::session::store::SessionStore::open(&store_path)
        .await
        .map_err(|e| e.to_string())?;

    if is_new_session {
        store
            .create_session(
                &active_id,
                &workspace_root.to_string_lossy(),
                app_config.provider.model_id(),
                &format!("{:?}", app_config.provider.provider),
            )
            .await
            .map_err(|e| e.to_string())?;

        // Save custom title as the first line of the prompt
        let title_candidate = prompt.lines().next().unwrap_or(&prompt).trim();
        let title = if title_candidate.is_empty() {
            "New Chat".to_string()
        } else {
            title_candidate.chars().take(40).collect::<String>()
        };

        if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
            let session_file = paths.sessions_dir.join(format!("{}.json", active_id));
            let mut map = std::fs::read_to_string(&session_file)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            map.insert("id".to_string(), serde_json::json!(active_id));
            map.insert("title".to_string(), serde_json::json!(title));
            let _ = std::fs::write(&session_file, serde_json::to_string_pretty(&map).unwrap_or_default());
        }
    } else {
        // Existing session: if the workspace context changed (session moved between
        // general ↔ project), update the stored workspace to match the new context.
        // This keeps the session metadata file accurate for sidebar categorization.
        if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
            let session_file = paths.sessions_dir.join(format!("{}.json", active_id));
            if session_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&session_file) {
                    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(obj) = val.as_object_mut() {
                            let stored_ws = obj
                                .get("workspace")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let current_ws = workspace_root.to_string_lossy();

                            // Only rewrite if the workspace actually changed
                            if stored_ws != current_ws.as_ref() {
                                obj.insert(
                                    "workspace".to_string(),
                                    serde_json::json!(current_ws),
                                );
                                let _ = std::fs::write(
                                    &session_file,
                                    serde_json::to_string_pretty(&val).unwrap_or_default(),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let config = operon_rs::session::SessionConfig {
        provider_config: app_config.provider.clone(),
        policy: app_config.policy.clone(),
        project_dir: if workspace_root != app_config.paths.workspace_dir {
            Some(workspace_root.clone())
        } else {
            None
        },
        workspace_root,
        role: operon_rs::context::Role::Owner,
        tool_groups: operon_rs::session::SessionConfig::default_tool_groups(),
        compaction: operon_rs::context::CompactionConfig::with_context_window(app_config.provider.context_window()),
        store_path: Some(store_path.clone()),
        channel_instructions: None,
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<operon_rs::SessionEvent>(100);
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<operon_rs::SessionCommand>(20);

    // Save cmd_tx so user can cancel prompt
    if let Ok(mut lock) = ACTIVE_CMD_TX.lock() {
        *lock = Some(cmd_tx.clone());
    }

    let mut runner = operon_rs::session::SessionRunner::new(config, event_tx, cmd_rx)
        .await
        .map_err(|e| e.to_string())?;

    if !is_new_session {
        if let Ok(history) = store.load_full_history(&active_id).await {
            let turns = store.load_turns(&active_id).await.unwrap_or_default();
            let last_tokens = store.get_last_token_count(&active_id).await.ok().flatten();
            if !history.is_empty() {
                runner.set_history(history, turns.len(), last_tokens);
            }
        }
    }

    // Process file attachments
    let mut file_paths = Vec::new();
    for att in attachments {
        file_paths.push(PathBuf::from(att.path));
    }

    let run_id = active_id.clone();
    let app_handle_done = app_handle.clone();
    let prompt_text = prompt.clone();
    let cmd_tx_events = cmd_tx.clone();
    let run_id_events = run_id.clone();

    // Spawn background task for streaming events to webview & registering permissions
    tokio::spawn(async move {
        let events_task = tokio::spawn(async move {
            let hook = crate::shared::channels_manager::create_channel_event_hook();
            while let Some(event) = event_rx.recv().await {
                hook(&run_id_events, &event, &cmd_tx_events);
            }
        });

        let run_result = runner.run(prompt_text, Vec::new(), file_paths).await;
        if let Err(e) = run_result {
            eprintln!("[Operon GUI] SessionRunner run error: {}", e);
            let _ = app_handle_done.emit("agent-error", e.to_string());
        }

        // Explicitly drop runner to close its internal event_tx channel
        drop(runner);

        // Wait for all buffered events to be dispatched to webview
        let _ = events_task.await;

        // Clear active command sender
        if let Ok(mut lock) = ACTIVE_CMD_TX.lock() {
            *lock = None;
        }

        let _ = app_handle_done.emit("agent-finished", &run_id);
    });

    Ok(active_id)
}

/// Cancels an in-flight prompt turn immediately.
#[tauri::command]
pub async fn cancel_prompt() -> Result<(), String> {
    let cmd_tx_opt = if let Ok(lock) = ACTIVE_CMD_TX.lock() {
        lock.clone()
    } else {
        None
    };

    if let Some(cmd_tx) = cmd_tx_opt {
        let _ = cmd_tx.send(operon_rs::SessionCommand::Cancel).await;
    }
    Ok(())
}

/// Approves a pending tool permission request across GUI sessions or background channel sessions.
#[tauri::command]
pub async fn approve_permission(permission_id: String) -> Result<(), String> {
    if let Ok(true) = crate::shared::channels_manager::dispatch_permission_decision(&permission_id, true).await {
        return Ok(());
    }

    let cmd_tx_opt = if let Ok(lock) = ACTIVE_CMD_TX.lock() {
        lock.clone()
    } else {
        None
    };

    if let Some(cmd_tx) = cmd_tx_opt {
        let _ = cmd_tx
            .send(operon_rs::SessionCommand::Approve {
                id: permission_id,
            })
            .await;
    }
    Ok(())
}

/// Denies a pending tool permission request across GUI sessions or background channel sessions.
#[tauri::command]
pub async fn deny_permission(permission_id: String) -> Result<(), String> {
    if let Ok(true) = crate::shared::channels_manager::dispatch_permission_decision(&permission_id, false).await {
        return Ok(());
    }

    let cmd_tx_opt = if let Ok(lock) = ACTIVE_CMD_TX.lock() {
        lock.clone()
    } else {
        None
    };

    if let Some(cmd_tx) = cmd_tx_opt {
        let _ = cmd_tx
            .send(operon_rs::SessionCommand::Deny {
                id: permission_id,
            })
            .await;
    }
    Ok(())
}

/// Truncates a session to a target turn index and resubmits an edited prompt.
#[tauri::command]
pub async fn edit_and_submit_prompt(
    app_handle: AppHandle,
    session_id: String,
    prompt: String,
    target_turn_index: usize,
    workspace_path: Option<String>,
) -> Result<String, String> {
    if prompt.trim().is_empty() {
        return Err("Prompt content cannot be empty".to_string());
    }

    if session_id.trim().is_empty() {
        return Err("Session ID cannot be empty".to_string());
    }

    // 1. Cancel any active prompt execution before rewinding history
    let cmd_tx_opt = if let Ok(lock) = ACTIVE_CMD_TX.lock() {
        lock.clone()
    } else {
        None
    };

    if let Some(cmd_tx) = cmd_tx_opt {
        let _ = cmd_tx.send(operon_rs::SessionCommand::Cancel).await;
    }

    let app_config = operon_rs::load().map_err(|e| e.to_string())?;

    // 2. Resolve workspace root from active context
    let workspace_root = match workspace_path {
        Some(ref p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => app_config.paths.workspace_dir.clone(),
    };

    let _ = std::fs::create_dir_all(&workspace_root);
    let _ = std::env::set_current_dir(&workspace_root);

    let store_path = app_config.paths.session_db(&session_id);
    let store = operon_rs::session::store::SessionStore::open(&store_path)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Truncate persistent store turns starting from target_turn_index
    store
        .truncate_turns(&session_id, target_turn_index)
        .await
        .map_err(|e| e.to_string())?;

    let config = operon_rs::session::SessionConfig {
        provider_config: app_config.provider.clone(),
        policy: app_config.policy.clone(),
        project_dir: if workspace_root != app_config.paths.workspace_dir {
            Some(workspace_root.clone())
        } else {
            None
        },
        workspace_root,
        role: operon_rs::context::Role::Owner,
        tool_groups: operon_rs::session::SessionConfig::default_tool_groups(),
        compaction: operon_rs::context::CompactionConfig::with_context_window(app_config.provider.context_window()),
        store_path: Some(store_path.clone()),
        channel_instructions: None,
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<operon_rs::SessionEvent>(100);
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<operon_rs::SessionCommand>(20);

    if let Ok(mut lock) = ACTIVE_CMD_TX.lock() {
        *lock = Some(cmd_tx.clone());
    }

    let mut runner = operon_rs::session::SessionRunner::new(config, event_tx, cmd_rx)
        .await
        .map_err(|e| e.to_string())?;

    // 4. Load truncated history up to target_turn_index into runner memory
    if let Ok(history) = store.load_full_history(&session_id).await {
        let turns = store.load_turns(&session_id).await.unwrap_or_default();
        let last_tokens = store.get_last_token_count(&session_id).await.ok().flatten();
        if !history.is_empty() {
            runner.set_history(history, turns.len(), last_tokens);
        }
    }

    let run_id = session_id.clone();
    let app_handle_done = app_handle.clone();
    let prompt_text = prompt.clone();
    let cmd_tx_events = cmd_tx.clone();
    let run_id_events = run_id.clone();

    // 5. Spawn background task for streaming events to webview & registering permissions
    tokio::spawn(async move {
        let events_task = tokio::spawn(async move {
            let hook = crate::shared::channels_manager::create_channel_event_hook();
            while let Some(event) = event_rx.recv().await {
                hook(&run_id_events, &event, &cmd_tx_events);
            }
        });

        let run_result = runner.run(prompt_text, Vec::new(), Vec::new()).await;
        if let Err(e) = run_result {
            eprintln!("[Operon GUI] Edit SessionRunner run error: {}", e);
            let _ = app_handle_done.emit("agent-error", e.to_string());
        }

        // Explicitly drop runner to close event_tx channel
        drop(runner);

        // Wait for all buffered events to be dispatched to webview
        let _ = events_task.await;

        // Clear active command sender
        if let Ok(mut lock) = ACTIVE_CMD_TX.lock() {
            *lock = None;
        }

        let _ = app_handle_done.emit("agent-finished", &run_id);
    });

    Ok(session_id)
}

