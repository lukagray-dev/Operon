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

use operon_channels_whatsapp::auth::WhatsAppAuth;
use operon_channels_whatsapp::client::WhatsAppClient;
use operon_channels_whatsapp::config::WhatsAppConfig;
use operon_channels_whatsapp::service::WhatsAppService;
use operon_channels_whatsapp::types::ContactId;
use operon_channels_whatsapp::{DeviceStore, RusqliteStore};

/// Static storage handle for the filesystem watcher to ensure it remains alive.
static WHATSAPP_WATCHER: std::sync::Mutex<Option<RecommendedWatcher>> = std::sync::Mutex::new(None);

/// Query WhatsApp contacts and session JSON files from disk and construct Slint SidebarProject DTOs.
pub fn load_whatsapp_sidebar_data() -> Vec<SidebarProject> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base_sessions = home.join(".operon").join("sessions").join("whatsapp");

    if !base_sessions.exists() {
        return Vec::new();
    }

    let mut projects = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&base_sessions) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let contact_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown Contact")
                    .to_string();

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
                        name: contact_name.into(),
                        workspace: path.to_string_lossy().to_string().into(),
                        conversations: ModelRc::from(Rc::new(VecModel::from(conv_model))),
                    });
                }
            }
        }
    }

    projects
}

/// Register WhatsApp sidebar setup and session selection actions.
/// Also handles auto-reconnect if credentials exist from a prior pairing
/// and spawns a filesystem watcher for live sidebar & chat content updates.
pub fn wire_whatsapp(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();

    // Populate initial WhatsApp contacts in sidebar
    let whatsapp_data = load_whatsapp_sidebar_data();
    window.set_sidebar_whatsapp_contacts(ModelRc::from(Rc::new(VecModel::from(whatsapp_data))));

    // Callback: WhatsApp session clicked
    window.on_sidebar_whatsapp_session_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |session_id: slint::SharedString, contact_number: slint::SharedString| {
            if let Some(win) = window_weak.upgrade() {
                println!(
                    "[operon-gui][whatsapp-sidebar] Clicked session {} for contact {}",
                    session_id, contact_number
                );

                // Set read-only posture for WhatsApp sessions
                win.set_is_read_only_session(true);

                let session_path = dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".operon")
                    .join("sessions")
                    .join("whatsapp")
                    .join(contact_number.as_str())
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

    // ── BUG 2: Filesystem Watcher for Live Refresh ───────────────────────────
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base_sessions = home.join(".operon").join("sessions").join("whatsapp");

    // Ensure session directory exists so watcher initialization succeeds
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
            if let Ok(mut guard) = WHATSAPP_WATCHER.lock() {
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

            // Debounce 250ms to aggregate rapid file writes
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;

            // Drain any buffered paths in channel
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
                    // 1. Refresh sidebar whatsapp contacts
                    let data = load_whatsapp_sidebar_data();
                    win.set_sidebar_whatsapp_contacts(ModelRc::from(Rc::new(VecModel::from(data))));

                    // 2. Refresh active session chat content if affected
                    let active_id = win.get_active_session_id().to_string();
                    if !active_id.is_empty() && active_id.starts_with("wa-") {
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
    let default_auth = home
        .join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("auth");
    let auth_checker = WhatsAppAuth::new(default_auth.clone());

    if auth_checker.has_credentials() {
        eprintln!(
            "[operon-gui][whatsapp-auto] Found existing credentials at {:?}. \
             Starting auto-reconnect...",
            default_auth
        );

        tokio::spawn(async move {
            let session_path = default_auth.join("session.db");
            let owner_number = if let Ok(storage) = RusqliteStore::new(&session_path) {
                if let Ok(Some(core_device)) = storage.load().await {
                    core_device.pn.as_ref().map(|jid| ContactId::new(&jid.user))
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(ref owner) = owner_number {
                eprintln!(
                    "[operon-gui][whatsapp-auto] Resolved owner_number: {}",
                    owner
                );
            }

            let config = WhatsAppConfig {
                enabled: true,
                owner_number,
                allowlist: vec![],
                auth_dir: Some(default_auth),
                workspace_dir: None,
            };

            let client = Arc::new(WhatsAppClient::new(&config));

            let app_config = match operon_rs::load() {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!(
                        "[operon-gui][whatsapp-auto] Failed to load AppConfig: {}. \
                         Auto-reconnect aborted.",
                        e
                    );
                    return;
                }
            };

            let service = WhatsAppService::new(client, config, app_config);

            if let Err(e) = service.run().await {
                eprintln!("[operon-gui][whatsapp-auto] WhatsAppService error: {}", e);
            }
        });
    }
}
