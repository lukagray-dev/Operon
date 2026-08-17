//! Filesystem sessions directory watcher using `notify`.
//!
//! Watches `~/.operon/sessions/` recursively for changes (WhatsApp chats, Telegram messages,
//! general chats, project sessions) and emits `sessions-changed` events with debounced aggregation.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use notify::{Config as NotifyConfig, Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

/// Static storage handle for the filesystem watcher to ensure it remains active.
static SESSIONS_WATCHER: Mutex<Option<RecommendedWatcher>> = Mutex::new(None);

/// Initializes the background watcher for `~/.operon/sessions/`.
pub fn init_sessions_watcher(app_handle: AppHandle) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let sessions_dir = home.join(".operon").join("sessions");

    if !sessions_dir.exists() {
        let _ = std::fs::create_dir_all(&sessions_dir);
    }

    let (tx, mut rx) = tauri::async_runtime::channel::<PathBuf>(100);

    let watcher_res = RecommendedWatcher::new(
        move |res: Result<NotifyEvent, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let _ = tx.blocking_send(path);
                }
            }
        },
        NotifyConfig::default(),
    );

    match watcher_res {
        Ok(mut watcher) => {
            if let Err(e) = watcher.watch(&sessions_dir, RecursiveMode::Recursive) {
                eprintln!("[operon-gui][watcher] Failed to watch sessions directory: {}", e);
                return;
            }

            if let Ok(mut lock) = SESSIONS_WATCHER.lock() {
                *lock = Some(watcher);
            }

            // Spawn consumer loop on Tauri async runtime
            tauri::async_runtime::spawn(async move {
                let mut modified_session_ids = HashSet::new();

                while let Some(path) = rx.recv().await {
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            modified_session_ids.insert(stem.to_string());
                        }
                    }

                    // Debounce 250ms to aggregate rapid burst writes
                    tokio::time::sleep(Duration::from_millis(250)).await;

                    // Drain any additional buffered paths
                    while let Ok(path) = rx.try_recv() {
                        if path.extension().and_then(|e| e.to_str()) == Some("json") {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                modified_session_ids.insert(stem.to_string());
                            }
                        }
                    }

                    let session_ids: Vec<String> = std::mem::take(&mut modified_session_ids)
                        .into_iter()
                        .collect();

                    // Emit event to frontend
                    let _ = app_handle.emit("sessions-changed", &session_ids);
                }
            });
        }
        Err(e) => {
            eprintln!("[operon-gui][watcher] Failed to initialize watcher: {}", e);
        }
    }
}
