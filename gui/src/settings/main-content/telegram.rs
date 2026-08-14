//! Controller for the Telegram channel settings page.
//!
//! This module wires the callbacks of the `TelegramSetup` Slint component:
//! - Loads Telegram config settings.
//! - Validates and saves bot token, Owner chat ID, allowlist, and workspace directory.
//! - Executes test connection requests via `getMe`.
//! - Spawns TelegramClient and TelegramService when user connects.

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex as StdMutex};

use crate::state::AppState;
use operon_rs::channels::telegram::client::TelegramClient;
use operon_rs::channels::telegram::config::TelegramConfig;
use operon_rs::channels::telegram::service::TelegramService;
use operon_rs::channels::telegram::types::ChatId;
use operon_rs::channels::telegram::ConnectionStatus;

/// Type alias for the shared client handle used across Slint callbacks.
type ClientHandle = Arc<StdMutex<Option<Arc<TelegramClient>>>>;

/// Registers callbacks for the Telegram channel settings panel.
pub fn wire_telegram_settings(window: &crate::SettingsWindow, _state: Rc<RefCell<AppState>>) {
    let weak_window = window.as_weak();

    // ── 1. Initial State Loading ─────────────────────────────────────────────
    let default_ws = TelegramConfig::default().resolved_workspace_dir();

    window.set_telegram_resolved_workspace_dir_placeholder(
        default_ws.to_string_lossy().as_ref().into(),
    );
    window.set_telegram_connection_status("Disconnected".into());

    // Check policy coverage for current workspace directory setting
    check_and_update_policy_coverage(window);

    let client_handle: ClientHandle = Arc::new(StdMutex::new(None));

    // ── 2. Handle Save & Connect Clicked ─────────────────────────────────────
    window.on_telegram_save_clicked({
        let weak_window = weak_window.clone();
        let client_handle = client_handle.clone();
        move |bot_token_str, owner_id_str, allowlist_model| {
            let token = if bot_token_str.trim().is_empty() {
                None
            } else {
                Some(bot_token_str.trim().to_string())
            };

            let owner_chat_id = if owner_id_str.trim().is_empty() {
                None
            } else {
                match owner_id_str.trim().parse::<i64>() {
                    Ok(id) => Some(ChatId::new(id)),
                    Err(_) => {
                        if let Some(win) = weak_window.upgrade() {
                            win.set_telegram_connection_status("Error: Owner Chat ID must be a valid integer".into());
                        }
                        return;
                    }
                }
            };

            let allowlist: Vec<ChatId> = allowlist_model
                .iter()
                .filter_map(|s| s.trim().parse::<i64>().ok().map(ChatId::new))
                .collect();

            let workspace_dir_str = weak_window
                .upgrade()
                .map(|w| w.get_telegram_workspace_dir().to_string())
                .unwrap_or_default();
            let workspace_dir = if workspace_dir_str.trim().is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(workspace_dir_str.trim()))
            };

            let config = TelegramConfig {
                enabled: true,
                bot_token: token.clone(),
                owner_chat_id,
                allowlist,
                workspace_dir,
                poll_interval_secs: Some(30),
            };

            if token.is_none() {
                if let Some(win) = weak_window.upgrade() {
                    win.set_telegram_connection_status("Error: Bot Token required".into());
                }
                return;
            }

            println!(
                "[operon-gui][telegram-settings] Saving & connecting Telegram config (owner_chat_id: {:?})",
                owner_chat_id
            );

            let client = Arc::new(TelegramClient::new(config.clone()));
            if let Ok(mut guard) = client_handle.lock() {
                *guard = Some(client.clone());
            }

            if let Some(win) = weak_window.upgrade() {
                win.set_telegram_connection_status("Connecting".into());
                win.set_telegram_test_status_message("".into());
            }

            tokio::spawn({
                let weak = weak_window.clone();
                let client = client.clone();
                let tg_config = config.clone();
                async move {
                    spawn_status_poller(weak.clone(), client.clone());

                    if let Err(e) = client.connect().await {
                        let err_str = e.to_string();
                        let weak_err = weak.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(win) = weak_err.upgrade() {
                                win.set_telegram_connection_status(format!("Error: {}", err_str).into());
                            }
                        })
                        .ok();
                    } else {
                        if let Ok(app_config) = operon_rs::load() {
                            let service = TelegramService::new(client.clone(), tg_config, app_config);
                            tokio::spawn(async move {
                                if let Err(e) = service.run().await {
                                    eprintln!("[operon-gui][telegram-settings] TelegramService error: {}", e);
                                }
                            });
                        }
                    }
                }
            });
        }
    });

    // ── 3. Handle Test Connection Clicked ────────────────────────────────────
    window.on_telegram_test_connection_clicked({
        let weak_window = weak_window.clone();
        move || {
            let weak = weak_window.clone();
            let bot_token_str = weak
                .upgrade()
                .map(|w| w.get_telegram_bot_token().to_string())
                .unwrap_or_default();

            if bot_token_str.trim().is_empty() {
                if let Some(win) = weak.upgrade() {
                    win.set_telegram_test_status_message(
                        "❌ Please enter a bot token first".into(),
                    );
                }
                return;
            }

            let config = TelegramConfig {
                enabled: true,
                bot_token: Some(bot_token_str.trim().to_string()),
                owner_chat_id: None,
                allowlist: Vec::new(),
                workspace_dir: None,
                poll_interval_secs: Some(30),
            };

            if let Some(win) = weak.upgrade() {
                win.set_telegram_test_status_message("Testing bot token via getMe...".into());
            }

            tokio::spawn(async move {
                let test_client = TelegramClient::new(config);
                match test_client.connect().await {
                    Ok(_) => {
                        slint::invoke_from_event_loop(move || {
                            if let Some(win) = weak.upgrade() {
                                win.set_telegram_test_status_message(
                                    "✓ Bot token is valid and active!".into(),
                                );
                            }
                        })
                        .ok();
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        slint::invoke_from_event_loop(move || {
                            if let Some(win) = weak.upgrade() {
                                win.set_telegram_test_status_message(
                                    format!("❌ Token test failed: {}", err_str).into(),
                                );
                            }
                        })
                        .ok();
                    }
                }
            });
        }
    });

    // ── 4. Handle Browse Workspace Directory Clicked ────────────────────────
    window.on_telegram_browse_workspace_dir_clicked({
        let weak_window = weak_window.clone();
        let default_ws = default_ws.clone();
        move || {
            if let Some(win) = weak_window.upgrade() {
                let current_val = win.get_telegram_workspace_dir().to_string();
                let starting_dir = if current_val.trim().is_empty() {
                    default_ws.clone()
                } else {
                    std::path::PathBuf::from(current_val.trim())
                };

                let mut dialog = rfd::FileDialog::new();
                if starting_dir.exists() {
                    dialog = dialog.set_directory(&starting_dir);
                }

                if let Some(folder) = dialog.pick_folder() {
                    let folder_str = folder.to_string_lossy().to_string();
                    win.set_telegram_workspace_dir(folder_str.into());
                    check_and_update_policy_coverage(&win);
                }
            }
        }
    });

    // ── 5. Handle Add Allowlist ID ───────────────────────────────────────────
    window.on_telegram_add_allowlist(add_allowlist_handler(weak_window.clone()));

    // ── 6. Handle Remove Allowlist ID ────────────────────────────────────────
    window.on_telegram_remove_allowlist(remove_allowlist_handler(weak_window.clone()));
}

fn spawn_status_poller(weak: slint::Weak<crate::SettingsWindow>, client: Arc<TelegramClient>) {
    tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(300);
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            if tokio::time::Instant::now() > deadline {
                break;
            }

            let status = client.status().await;
            match status {
                ConnectionStatus::Connected => {
                    let weak2 = weak.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(win) = weak2.upgrade() {
                            win.set_telegram_connection_status("Connected".into());
                        }
                    })
                    .ok();
                    break;
                }
                ConnectionStatus::Error(ref err) => {
                    let err_msg = format!("Error: {}", err);
                    let weak2 = weak.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(win) = weak2.upgrade() {
                            win.set_telegram_connection_status(err_msg.into());
                        }
                    })
                    .ok();
                    break;
                }
                _ => {}
            }
        }
    });
}

fn add_allowlist_handler(
    weak_window: slint::Weak<crate::SettingsWindow>,
) -> impl FnMut(SharedString) {
    move |new_id| {
        if let Some(win) = weak_window.upgrade() {
            let current_model = win.get_telegram_allowlist();
            let mut list: Vec<SharedString> = current_model.iter().collect();
            if !new_id.trim().is_empty() {
                list.push(new_id.into());
                win.set_telegram_allowlist(ModelRc::from(Rc::new(VecModel::from(list))));
            }
        }
    }
}

fn remove_allowlist_handler(weak_window: slint::Weak<crate::SettingsWindow>) -> impl FnMut(i32) {
    move |idx| {
        if let Some(win) = weak_window.upgrade() {
            let current_model = win.get_telegram_allowlist();
            let mut list: Vec<SharedString> = current_model.iter().collect();
            let index = idx as usize;
            if index < list.len() {
                list.remove(index);
                win.set_telegram_allowlist(ModelRc::from(Rc::new(VecModel::from(list))));
            }
        }
    }
}

fn check_and_update_policy_coverage(win: &crate::SettingsWindow) {
    let ws_input = win.get_telegram_workspace_dir().to_string();
    let resolved_path = if ws_input.trim().is_empty() {
        TelegramConfig::default().resolved_workspace_dir()
    } else {
        std::path::PathBuf::from(ws_input.trim())
    };
    let canonical = std::fs::canonicalize(&resolved_path).unwrap_or_else(|_| resolved_path.clone());
    let is_covered = if let Ok(app_config) = operon_rs::load() {
        app_config.policy.any_directory_covers(&canonical)
    } else {
        false
    };
    win.set_telegram_is_policy_covered(is_covered);
}
