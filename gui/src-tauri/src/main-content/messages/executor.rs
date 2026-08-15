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

    let workspace_root = match workspace_path {
        Some(ref p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => app_config.paths.workspace_dir.clone(),
    };

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
        tool_groups: vec!["fs".into(), "shell".into(), "web".into(), "todo".into()],
        compaction: operon_rs::context::CompactionConfig::default(),
        store_path: Some(store_path.clone()),
        channel_instructions: None,
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<operon_rs::SessionEvent>(100);
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<operon_rs::SessionCommand>(20);

    // Save cmd_tx so user can cancel prompt
    if let Ok(mut lock) = ACTIVE_CMD_TX.lock() {
        *lock = Some(cmd_tx);
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
    let app_handle_events = app_handle.clone();
    let app_handle_done = app_handle.clone();
    let prompt_text = prompt.clone();

    // Spawn background task for streaming events to webview
    tokio::spawn(async move {
        let events_task = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let _ = app_handle_events.emit("agent-event", &event);
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

/// Approves a pending tool permission request.
#[tauri::command]
pub async fn approve_permission(permission_id: String) -> Result<(), String> {
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

/// Denies a pending tool permission request.
#[tauri::command]
pub async fn deny_permission(permission_id: String) -> Result<(), String> {
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
