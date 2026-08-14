use notify::{
    Config as NotifyConfig, Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crate::state::AppState;
use crate::{SidebarConversation, SidebarProject};

use operon_rs::channels::telegram::client::TelegramClient;
use operon_rs::channels::telegram::config::TelegramConfig;
use operon_rs::channels::telegram::service::TelegramService;


/// Static storage handle for the filesystem watcher to ensure it remains alive.
static TELEGRAM_WATCHER: std::sync::Mutex<Option<RecommendedWatcher>> = std::sync::Mutex::new(None);

/// Query Telegram chat IDs and session JSON files from disk and construct Slint SidebarProject DTOs.
pub fn load_telegram_sidebar_data() -> Vec<SidebarProject> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base_sessions = home.join(".operon").join("sessions").join("telegram");

    if !base_sessions.exists() {
        return Vec::new();
    }

    let mut projects = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&base_sessions) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let chat_id_str = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown Chat")
                    .to_string();

                let project_title = format!("Telegram: {}", chat_id_str);
                let mut conversations = Vec::new();

                if let Ok(sess_entries) = std::fs::read_dir(&path) {
                    for sess_entry in sess_entries.flatten() {
                        let sess_path = sess_entry.path();
                        if sess_path.is_file()
                            && sess_path.extension().and_then(|e| e.to_str()) == Some("json")
                        {
                            let session_id = sess_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();

                            let title =
                                format!("Session {}", &session_id[..session_id.len().min(8)]);

                            conversations.push(SidebarConversation {
                                id: session_id.into(),
                                title: title.into(),
                            });
                        }
                    }
                }

                if !conversations.is_empty() {
                    let conv_model: Vec<SidebarConversation> = conversations;
                    projects.push(SidebarProject {
                        name: project_title.into(),
                        workspace: path.to_string_lossy().to_string().into(),
                        conversations: ModelRc::from(Rc::new(VecModel::from(conv_model))),
                    });
                }
            }
        }
    }

    projects
}

/// Register Telegram sidebar setup and session selection actions.
/// Also handles auto-reconnect if a bot token is present in configuration
/// and spawns a filesystem watcher for live sidebar & chat content updates.
pub fn wire_telegram(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();

    // Populate initial Telegram chats in sidebar
    let telegram_data = load_telegram_sidebar_data();
    window.set_sidebar_telegram_contacts(ModelRc::from(Rc::new(VecModel::from(telegram_data))));

    // Callback: Telegram session clicked
    window.on_sidebar_telegram_session_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |session_id: slint::SharedString, chat_id: slint::SharedString| {
            if let Some(win) = window_weak.upgrade() {
                println!(
                    "[operon-gui][telegram-sidebar] Clicked session {} for chat {}",
                    session_id, chat_id
                );

                // Set read-only posture for Telegram sessions
                win.set_is_read_only_session(true);

                // Extract raw ChatId from display title if prefixed with "Telegram: "
                let raw_chat_id = chat_id.trim_start_matches("Telegram: ").trim();

                let session_path = dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".operon")
                    .join("sessions")
                    .join("telegram")
                    .join(raw_chat_id)
                    .join(format!("{}.json", session_id));

                // Load chat session messages
                crate::left_sidebar::load_chat_session(
                    &win,
                    session_id.as_str(),
                    None,
                    Some(session_path),
                    &app_state,
                );
            }
        }
    });

    // ── Filesystem Watcher for Live Refresh ───────────────────────────
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base_sessions = home.join(".operon").join("sessions").join("telegram");

    let _ = std::fs::create_dir_all(&base_sessions);

    let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel::<PathBuf>(100);

    let watcher_res = RecommendedWatcher::new(
        move |res: notify::Result<NotifyEvent>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let _ = watcher_tx.try_send(path);
                }
            }
        },
        NotifyConfig::default(),
    );

    if let Ok(mut watcher) = watcher_res {
        if watcher
            .watch(&base_sessions, RecursiveMode::Recursive)
            .is_ok()
        {
            if let Ok(mut guard) = TELEGRAM_WATCHER.lock() {
                *guard = Some(watcher);
            }
        }
    }

    // Spawn debounced filesystem watcher processor task
    let window_weak_for_watcher = window_weak.clone();

    tokio::spawn(async move {
        let mut modified_session_ids = std::collections::HashSet::new();

        while let Some(path) = watcher_rx.recv().await {
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    modified_session_ids.insert(stem.to_string());
                }
            }

            // Debounce 250ms
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;

            while let Ok(path) = watcher_rx.try_recv() {
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        modified_session_ids.insert(stem.to_string());
                    }
                }
            }

            let session_ids = std::mem::take(&mut modified_session_ids);
            let window_weak = window_weak_for_watcher.clone();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = window_weak.upgrade() {
                    // 1. Refresh sidebar telegram contacts
                    let data = load_telegram_sidebar_data();
                    win.set_sidebar_telegram_contacts(ModelRc::from(Rc::new(VecModel::from(data))));

                    // 2. Refresh active session chat content if affected
                    let active_id = win.get_active_session_id().to_string();
                    if !active_id.is_empty() && active_id.starts_with("tg-") {
                        if session_ids.is_empty() || session_ids.contains(&active_id) {
                            let win_weak_inner = win.as_weak();
                            let active_id_clone = active_id.clone();
                            tokio::spawn(async move {
                                if let Ok((
                                    title,
                                    raw_messages,
                                    last_token_count,
                                    context_window_opt,
                                )) = crate::left_sidebar::conversation::load_session_history(
                                    &active_id_clone,
                                    None,
                                )
                                .await
                                {
                                    let context_window = context_window_opt.unwrap_or(128_000);
                                    let utilization = if context_window > 0 {
                                        last_token_count as f32 / context_window as f32
                                    } else {
                                        0.0
                                    };
                                    let context_text =
                                        crate::main_content::input::context::format_tokens(
                                            last_token_count as i32,
                                            context_window as i32,
                                        );

                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = win_weak_inner.upgrade() {
                                            if ui.get_active_session_id() != active_id_clone {
                                                return;
                                            }
                                            crate::main_content::title::set_session_title(
                                                &ui, &title,
                                            );
                                            let slint_messages: Vec<crate::ChatMessage> = raw_messages
                                                .into_iter()
                                                .map(|(is_user, text, items, time_str)| {
                                                    let elements = crate::main_content::markdown::to_slint_elements(items);
                                                    crate::ChatMessage {
                                                        id: "".into(),
                                                        is_user,
                                                        text: text.into(),
                                                        time: time_str.into(),
                                                        markdown_elements: slint::ModelRc::from(Rc::new(
                                                            slint::VecModel::from(elements),
                                                        )),
                                                        reasoning_text: "".into(),
                                                        is_thinking: false,
                                                    }
                                                })
                                                .collect();

                                            ui.set_chat_messages(slint::ModelRc::from(Rc::new(
                                                slint::VecModel::from(slint_messages),
                                            )));
                                            ui.set_context_usage(utilization);
                                            ui.set_tokens_used(last_token_count as i32);
                                            ui.set_tokens_total(context_window as i32);
                                            ui.set_context_text(context_text.into());
                                            ui.set_is_loading_session(false);
                                        }
                                    });
                                }
                            });
                        }
                    }
                }
            });
        }
    });

    // ── Auto-reconnect on startup ────────────────────────────────────────────
    let bot_token_env = std::env::var("TELEGRAM_BOT_TOKEN").ok();
    if let Some(token) = bot_token_env {
        if !token.trim().is_empty() {
            eprintln!("[operon-gui][telegram-auto] Found TELEGRAM_BOT_TOKEN env var. Starting auto-reconnect...");
            let config = TelegramConfig {
                enabled: true,
                bot_token: Some(token),
                owner_chat_id: None,
                allowlist: Vec::new(),
                workspace_dir: None,
                poll_interval_secs: Some(30),
            };

            let client = Arc::new(TelegramClient::new(config.clone()));

            if let Ok(app_config) = operon_rs::load() {
                let service = TelegramService::new(client, config, app_config);
                tokio::spawn(async move {
                    if let Err(e) = service.run().await {
                        eprintln!("[operon-gui][telegram-auto] TelegramService error: {}", e);
                    }
                });
            }
        }
    }
}
