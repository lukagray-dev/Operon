//! Filesystem channels sessions directory watcher using `notify`.
//!
//! Exclusively watches external channel session directories (`~/.operon/sessions/whatsapp/`
//! and `~/.operon/sessions/telegram/`) for changes from background services, emitting
//! `sessions-changed` events. General chats and project sessions are managed directly in-memory
//! by the GUI and are not monitored by `notify`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use notify::{Config as NotifyConfig, Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

/// Static storage handle for the filesystem watcher to ensure it remains active.
static SESSIONS_WATCHER: Mutex<Option<RecommendedWatcher>> = Mutex::new(None);

/// Initializes the background watcher exclusively for channel directories (`whatsapp` and `telegram`).
pub fn init_sessions_watcher(app_handle: AppHandle) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let wa_dir = home.join(".operon").join("sessions").join("whatsapp");
    let tg_dir = home.join(".operon").join("sessions").join("telegram");

    let _ = std::fs::create_dir_all(&wa_dir);
    let _ = std::fs::create_dir_all(&tg_dir);

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
            // Watch only external channel subdirectories
            let _ = watcher.watch(&wa_dir, RecursiveMode::Recursive);
            let _ = watcher.watch(&tg_dir, RecursiveMode::Recursive);

            if let Ok(mut lock) = SESSIONS_WATCHER.lock() {
                *lock = Some(watcher);
            }

            // Spawn consumer loop on Tauri async runtime
            tauri::async_runtime::spawn(async move {
                let mut modified_session_ids = HashSet::new();

                while let Some(path) = rx.recv().await {
                    let path_str = path.to_string_lossy();
                    let is_channel = path_str.contains("whatsapp") || path_str.contains("telegram");

                    if is_channel && path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            modified_session_ids.insert(stem.to_string());
                        }
                    }

                    // Debounce 250ms to aggregate rapid burst writes
                    tokio::time::sleep(Duration::from_millis(250)).await;

                    // Drain any additional buffered paths
                    while let Ok(path) = rx.try_recv() {
                        let path_str = path.to_string_lossy();
                        let is_channel = path_str.contains("whatsapp") || path_str.contains("telegram");

                        if is_channel && path.extension().and_then(|e| e.to_str()) == Some("json") {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                modified_session_ids.insert(stem.to_string());
                            }
                        }
                    }

                    if !modified_session_ids.is_empty() {
                        let session_ids: Vec<String> = std::mem::take(&mut modified_session_ids)
                            .into_iter()
                            .collect();

                        // Emit event to frontend
                        let _ = app_handle.emit("sessions-changed", &session_ids);
                    }
                }
            });
        }
        Err(e) => {
            eprintln!("[operon-gui][watcher] Failed to initialize channel watcher: {}", e);
        }
    }
}
