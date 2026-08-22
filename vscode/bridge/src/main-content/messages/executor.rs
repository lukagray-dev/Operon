//! Agent Prompt Execution and Background SessionRunner Management in Bridge.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::main_content::input::PendingAttachmentDto;
use crate::shared::AppState;

/// Global thread-safe reference to the active session command channel for cancellation and approval.
pub static ACTIVE_CMD_TX: Mutex<Option<tokio::sync::mpsc::Sender<operon_rs::SessionCommand>>> =
    Mutex::new(None);

/// Submits a user prompt to the agent SessionRunner.
pub async fn submit_prompt(
    _state: &Arc<AppState>,
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

    let workspace_root = match workspace_path {
        Some(ref p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => app_config.paths.workspace_dir.clone(),
    };

    let _ = std::fs::create_dir_all(&workspace_root);

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
            let _ = std::fs::write(
                &session_file,
                serde_json::to_string_pretty(&map).unwrap_or_default(),
            );
        }
    } else if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
        let session_file = paths.sessions_dir.join(format!("{}.json", active_id));
        if session_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(obj) = val.as_object_mut() {
                        let stored_ws = obj.get("workspace").and_then(|v| v.as_str()).unwrap_or("");
                        let current_ws = workspace_root.to_string_lossy();
                        if stored_ws != current_ws.as_ref() {
                            obj.insert("workspace".to_string(), serde_json::json!(current_ws));
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
        compaction: operon_rs::context::CompactionConfig::with_context_window(
            app_config.provider.context_window(),
        ),
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

    if !is_new_session {
        if let Ok(history) = store.load_full_history(&active_id).await {
            let turns = store.load_turns(&active_id).await.unwrap_or_default();
            let last_tokens = store.get_last_token_count(&active_id).await.ok().flatten();
            if !history.is_empty() {
                runner.set_history(history, turns.len(), last_tokens);
            }
        }
    }

    let mut file_paths = Vec::new();
    for att in attachments {
        file_paths.push(PathBuf::from(att.path));
    }

    let run_id = active_id.clone();
    let prompt_text = prompt.clone();
    let cmd_tx_events = cmd_tx.clone();
    let run_id_events = run_id.clone();

    tokio::spawn(async move {
        let events_task = tokio::spawn(async move {
            let hook = crate::shared::channels_manager::create_channel_event_hook();
            while let Some(event) = event_rx.recv().await {
                hook(&run_id_events, &event, &cmd_tx_events);
            }
        });

        let run_result = runner.run(prompt_text, Vec::new(), file_paths).await;
        if let Err(e) = run_result {
            eprintln!("[Operon Bridge] SessionRunner run error: {}", e);
            if let Some(state) = crate::shared::channels_manager::get_app_state() {
                state.emit_event("agent-error", e.to_string()).await;
            }
        }

        drop(runner);
        let _ = events_task.await;

        if let Ok(mut lock) = ACTIVE_CMD_TX.lock() {
            *lock = None;
        }

        if let Some(state) = crate::shared::channels_manager::get_app_state() {
            state.emit_event("agent-finished", &run_id).await;
        }
    });

    Ok(active_id)
}

/// Cancels an in-flight prompt turn immediately.
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

/// Approves a pending tool permission request.
pub async fn approve_permission(permission_id: String) -> Result<(), String> {
    if let Ok(true) =
        crate::shared::channels_manager::dispatch_permission_decision(&permission_id, true).await
    {
        return Ok(());
    }

    let cmd_tx_opt = if let Ok(lock) = ACTIVE_CMD_TX.lock() {
        lock.clone()
    } else {
        None
    };

    if let Some(cmd_tx) = cmd_tx_opt {
        let _ = cmd_tx
            .send(operon_rs::SessionCommand::Approve { id: permission_id })
            .await;
    }
    Ok(())
}

/// Denies a pending tool permission request.
pub async fn deny_permission(permission_id: String) -> Result<(), String> {
    if let Ok(true) =
        crate::shared::channels_manager::dispatch_permission_decision(&permission_id, false).await
    {
        return Ok(());
    }

    let cmd_tx_opt = if let Ok(lock) = ACTIVE_CMD_TX.lock() {
        lock.clone()
    } else {
        None
    };

    if let Some(cmd_tx) = cmd_tx_opt {
        let _ = cmd_tx
            .send(operon_rs::SessionCommand::Deny { id: permission_id })
            .await;
    }
    Ok(())
}

/// Truncates a session to a target turn index and resubmits an edited prompt.
pub async fn edit_and_submit_prompt(
    state: &Arc<AppState>,
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

    let cmd_tx_opt = if let Ok(lock) = ACTIVE_CMD_TX.lock() {
        lock.clone()
    } else {
        None
    };

    if let Some(cmd_tx) = cmd_tx_opt {
        let _ = cmd_tx.send(operon_rs::SessionCommand::Cancel).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let app_config = operon_rs::load().map_err(|e| e.to_string())?;
    let store_path = app_config.paths.session_db(&session_id);
    let store = operon_rs::session::store::SessionStore::open(&store_path)
        .await
        .map_err(|e| e.to_string())?;

    // Truncate persistent store turns starting from target_turn_index
    store
        .truncate_turns(&session_id, target_turn_index)
        .await
        .map_err(|e| e.to_string())?;

    submit_prompt(
        state,
        Some(session_id),
        prompt,
        Vec::new(),
        workspace_path,
    )
    .await
}
