//! Projects specific callback wiring and native filesystem dialog interactions.
//!
//! This module separates the general chats setup from project configuration.

use std::cell::RefCell;
use std::rc::Rc;
use rfd::{MessageDialog, MessageLevel, MessageButtons, MessageDialogResult};
use slint::{ComponentHandle, Model};

use crate::state::AppState;

/// Helper function to strip raw Windows UNC prefix for cleaner paths.
fn clean_unc_path(s: String) -> String {
    if s.starts_with(r"\\?\") {
        s[4..].to_string()
    } else {
        s
    }
}

/// Register project folder list operations, session configurations, and native dialogs.
pub fn wire_projects(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();

    // Callback 1: Project conversation clicked
    window.on_sidebar_project_conversation_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |session_id, proj_idx, conv_idx| {
            if let Some(win) = window_weak.upgrade() {
                win.set_active_project_index(proj_idx);
                win.set_active_conversation_index(conv_idx);
                win.set_active_chat_index(-1);
                
                // Read the workspace path dynamically from the projects array in Slint
                let mut project_path = None;
                if let Some(project) = win.get_sidebar_projects().row_data(proj_idx as usize) {
                    project_path = Some(project.workspace.to_string());
                }

                crate::left_sidebar::load_chat_session(&win, &session_id, project_path.as_deref(), &app_state);
            }
        }
    });

    // Callback 2: Start new session for a specific project folder
    window.on_sidebar_new_session_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |proj_path, proj_idx| {
            if let Some(win) = window_weak.upgrade() {
                println!("[operon-gui][sidebar-projects] Creating new project chat for workspace: {}", proj_path);
                {
                    let mut g_state = app_state.borrow_mut();
                    g_state.set_active_session_id(None);
                    g_state.set_current_project_dir(Some(proj_path.to_string()));
                }
                win.set_active_session_id("".into());
                win.set_active_project_index(proj_idx);
                win.set_active_conversation_index(-1);
                win.set_active_chat_index(-1);
                crate::main_content::title::set_session_title(&win, "New Chat");
                win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::default())));

                let app_config = operon_rs::config::load().ok();
                let context_window = app_config.as_ref().map(|c| c.provider.model.context_window).unwrap_or(128_000);
                win.set_context_usage(0.0);
                win.set_tokens_used(0);
                win.set_tokens_total(context_window as i32);
                win.set_context_text(crate::main_content::input::context::format_tokens(0, context_window as i32).into());
            }
        }
    });

    // Callback 3: Delete a project-bound conversation session
    window.on_sidebar_delete_conversation_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |session_id, proj_idx, conv_idx| {
            let confirmed = MessageDialog::new()
                .set_title("Delete Chat Session")
                .set_description("Are you sure you want to delete this chat session?")
                .set_level(MessageLevel::Warning)
                .set_buttons(MessageButtons::OkCancel)
                .show();

            if confirmed == MessageDialogResult::Ok {
                // Clear state on the main thread first
                let active_id = {
                    let mut g_state = app_state.borrow_mut();
                    if g_state.active_session_id() == Some(&session_id) {
                        g_state.set_active_session_id(None);
                    }
                    g_state.active_session_id().map(String::from)
                };

                let win_weak = window_weak.clone();
                let session_id_clone = session_id.clone();

                tokio::spawn(async move {
                    if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
                        let json_path = paths.session_db(&session_id_clone);
                        if json_path.exists() {
                            let _ = std::fs::remove_file(json_path);
                        }
                    }

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = win_weak.upgrade() {
                            if win.get_active_project_index() == proj_idx && win.get_active_conversation_index() == conv_idx {
                                crate::main_content::title::set_session_title(&win, "New Chat");
                                win.set_active_session_id("".into());
                            }
                            crate::left_sidebar::clear_sidebar_selection(&win);
                            crate::left_sidebar::refresh_sidebar(&win, active_id);
                        }
                    });
                });
            }
        }
    });

    // Callback 4: Delete a project folder and all its associated chat sessions
    window.on_sidebar_delete_project_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |proj_path, proj_idx| {
            let name = std::path::Path::new(&proj_path.to_string())
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&proj_path)
                .to_string();

            let confirmed = MessageDialog::new()
                .set_title("Delete Project")
                .set_description(&format!("Are you sure you want to delete project \"{}\"? This will delete all its chat sessions and remove it from the sidebar.", name))
                .set_level(MessageLevel::Warning)
                .set_buttons(MessageButtons::OkCancel)
                .show();

            if confirmed == MessageDialogResult::Ok {
                // Clear state on the main thread if the active project matches the deleted project!
                let (active_project_deleted, active_id) = {
                    let mut g_state = app_state.borrow_mut();
                    let deleted = if g_state.current_project_dir() == Some(&proj_path) {
                        g_state.set_active_session_id(None);
                        g_state.set_current_project_dir(None);
                        true
                    } else {
                        false
                    };
                    (deleted, g_state.active_session_id().map(String::from))
                };

                let win_weak = window_weak.clone();
                let proj_path_clone = proj_path.clone();

                tokio::spawn(async move {
                    // Find all sessions inside this project path
                    let mut session_ids_to_delete = Vec::new();
                    if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
                        let sessions_dir = &paths.sessions_dir;
                        if sessions_dir.exists() {
                            if let Ok(entries) = std::fs::read_dir(sessions_dir) {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if path.extension().map_or(false, |ext| ext == "json") {
                                        if let Ok(store) = operon_rs::session::store::SessionStore::open(&path).await {
                                            if let Ok(rows) = store.list_sessions().await {
                                                if let Some(row) = rows.first() {
                                                    let clean_ws = clean_unc_path(row.workspace.clone());
                                                    let clean_proj = clean_unc_path(proj_path_clone.to_string());
                                                    if clean_ws == clean_proj {
                                                        session_ids_to_delete.push(row.id.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Remove database files
                        for id in &session_ids_to_delete {
                            let json_path = paths.session_db(id);
                            if json_path.exists() {
                                let _ = std::fs::remove_file(json_path);
                            }
                        }
                    }

                    // Remove from allowed list in config.toml
                    let _ = operon_rs::config::remove_allowed_directory(&proj_path_clone.to_string());

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = win_weak.upgrade() {
                            if active_project_deleted {
                                crate::main_content::title::set_session_title(&win, "New Chat");
                                win.set_active_session_id("".into());
                            }
                            if win.get_active_project_index() == proj_idx {
                                crate::left_sidebar::clear_sidebar_selection(&win);
                            }
                            crate::left_sidebar::refresh_sidebar(&win, active_id);
                        }
                    });
                });
            }
        }
    });

    // Callback 5: Open Project folder picker (called from titlebar menu "Files" -> "Open project")
    window.on_open_project_requested({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move || {
            let picked_folder = rfd::FileDialog::new()
                .pick_folder();

            if let Some(path_buf) = picked_folder {
                let path_str = path_buf.to_string_lossy().to_string();
                println!("[operon-gui][sidebar-projects] User picked folder to open project: {}", path_str);

                // Update AppState directly on the main thread!
                let active_id = {
                    let mut g_state = app_state.borrow_mut();
                    g_state.set_active_session_id(None);
                    g_state.set_current_project_dir(Some(path_str.clone()));
                    g_state.active_session_id().map(String::from)
                };

                if let Some(win) = window_weak.upgrade() {
                    win.set_active_session_id("".into());
                }
                let win_weak = window_weak.clone();
                let path_str_clone = path_str.clone();

                tokio::spawn(async move {
                    // Add folder path to config.toml allowed list (idempotent)
                    if let Ok(_) = operon_rs::config::add_allowed_directory(&path_str_clone) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(win) = win_weak.upgrade() {
                                // Clear selections
                                crate::left_sidebar::clear_sidebar_selection(&win);
                                crate::main_content::title::set_session_title(&win, "New Chat");
                                crate::left_sidebar::refresh_sidebar(&win, active_id);
                            }
                        });
                    }
                });
            }
        }
    });
}
