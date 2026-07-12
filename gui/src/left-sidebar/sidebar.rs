//! Left Sidebar Controller for chats and projects.
//!
//! This module binds user interactions on the left sidebar navigation pane to the
//! `operon-rs` backend. It reads historical session JSON files from `~/.operon/sessions/*.json`,
//! categorizes them into project-bound vs general chats, handles folder selection via
//! native dialogs, and manages session deletions.

use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use slint::{ComponentHandle, ModelRc, VecModel, Model};
use rfd::{MessageDialog, MessageLevel, MessageButtons, MessageDialogResult};

use crate::state::AppState;

struct SessionRecord {
    id: String,
    created_at: i64,
    workspace: String,
    title: String,
    is_project: bool,
}

/// Helper function to strip raw Windows UNC prefix for cleaner paths.
fn clean_unc_path(s: String) -> String {
    if s.starts_with(r"\\?\") {
        s[4..].to_string()
    } else {
        s
    }
}

/// Asynchronously queries session JSON files, matches them against allowed projects,
/// constructs Slint models, and updates the Operon window.
fn refresh_sidebar(window: &crate::OperonWindow) {
    let window_weak = window.as_weak();

    tokio::spawn(async move {
        let run_refresh = async {
            let paths = operon_rs::config::OperonPaths::resolve()?;
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

            // Query configured workspace directories from config.toml
            let mut projects_list = Vec::new();
            if let Ok(allowed_dirs) = operon_rs::get_allowed_directories_list() {
                for dir in allowed_dirs.0 {
                    let cleaned = clean_unc_path(dir.clone());
                    if cleaned != default_workspace {
                        projects_list.push(dir);
                    }
                }
            }

            let mut sessions = Vec::new();
            if sessions_dir.exists() {
                let entries = std::fs::read_dir(sessions_dir)?;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "json") {
                        if let Ok(store) = operon_rs::session::store::SessionStore::open(&path).await {
                            if let Ok(rows) = store.list_sessions().await {
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
                                            .unwrap_or_else(|_| std::path::PathBuf::from(&row.workspace))
                                            .to_string_lossy()
                                            .to_string();
                                        clean_unc_path(p)
                                    };

                                    let is_project = session_workspace_canon != default_workspace;
                                    sessions.push(SessionRecord {
                                        id: row.id.clone(),
                                        created_at: row.created_at,
                                        workspace: row.workspace.clone(),
                                        title,
                                        is_project,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Sort newest first
            sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            // Separate into standalone chats vs project conversations
            let mut standalone_chats = Vec::new();
            let mut project_chats_map: HashMap<String, Vec<crate::SidebarConversation>> = HashMap::new();

            for p in &projects_list {
                project_chats_map.insert(p.clone(), Vec::new());
            }

            for s in sessions {
                if !s.is_project {
                    standalone_chats.push(crate::SidebarConversation {
                        id: s.id.clone().into(),
                        title: s.title.clone().into(),
                    });
                } else {
                    let entry_chats = project_chats_map.entry(s.workspace.clone()).or_insert_with(Vec::new);
                    entry_chats.push(crate::SidebarConversation {
                        id: s.id.clone().into(),
                        title: s.title.clone().into(),
                    });
                }
            }

            // Group project details to pass them thread-safely
            let mut projects_data = Vec::new();
            for p in projects_list {
                let name = std::path::Path::new(&p)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&p)
                    .to_string();

                let conversations = project_chats_map.remove(&p).unwrap_or_default();
                projects_data.push((name, p, conversations));
            }

            // Dispatch update to Slint main thread
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = window_weak.upgrade() {
                    ui.set_sidebar_chats(ModelRc::from(Rc::new(VecModel::from(standalone_chats))));
                    
                    let slint_projects: Vec<crate::SidebarProject> = projects_data
                        .into_iter()
                        .map(|(name, workspace, convs)| crate::SidebarProject {
                            name: name.into(),
                            workspace: workspace.into(),
                            conversations: ModelRc::from(Rc::new(VecModel::from(convs))),
                        })
                        .collect();
                    ui.set_sidebar_projects(ModelRc::from(Rc::new(VecModel::from(slint_projects))));
                }
            });

            anyhow::Ok(())
        }.await;

        if let Err(e) = run_refresh {
            eprintln!("[operon-gui][sidebar] Failed to query workspace sessions: {}", e);
        }
    });
}

/// Reset active conversation selection indices in Slint
fn clear_sidebar_selection(window: &crate::OperonWindow) {
    window.set_active_chat_index(-1);
    window.set_active_project_index(-1);
    window.set_active_conversation_index(-1);
}

/// Handle selection and load messages of a chosen chat session.
fn load_chat_session(
    window: &crate::OperonWindow,
    session_id: &str,
    project_path: Option<&str>,
    app_state: &Rc<RefCell<AppState>>,
) {
    // Update global state variables
    {
        let mut state = app_state.borrow_mut();
        state.set_active_session_id(Some(session_id.to_string()));
        state.set_current_project_dir(project_path.map(String::from));
    }

    println!("[operon-gui][sidebar] Selected session: {}, project: {:?}", session_id, project_path);

    // Retrieve conversation title and update Slint title property
    let window_weak = window.as_weak();
    let session_id_str = session_id.to_string();
    
    tokio::spawn(async move {
        let _load_session = async {
            let paths = operon_rs::config::OperonPaths::resolve()?;
            let json_path = paths.session_db(&session_id_str);
            if json_path.exists() {
                let store = operon_rs::session::store::SessionStore::open(&json_path).await?;
                if let Ok(Some(first_msg)) = store.get_first_user_message_text(&session_id_str).await {
                    let mut clean_title = first_msg.replace('\n', " ").trim().to_string();
                    if clean_title.len() > 40 {
                        clean_title = format!("{}...", &clean_title[..40]);
                    }

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = window_weak.upgrade() {
                            ui.set_session_title(clean_title.into());
                        }
                    });
                }
            }
            anyhow::Ok(())
        }.await;
    });
}

/// Wire all sidebar event callbacks and fetch initial records.
pub fn wire_sidebar(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    // Trigger initial load
    refresh_sidebar(window);

    // Callback 1: Standalone chat clicked
    window.on_sidebar_chat_clicked({
        let window_weak = window.as_weak();
        let app_state = Rc::clone(&state);
        move |session_id, chat_idx| {
            if let Some(win) = window_weak.upgrade() {
                win.set_active_chat_index(chat_idx);
                win.set_active_project_index(-1);
                win.set_active_conversation_index(-1);
                load_chat_session(&win, &session_id, None, &app_state);
            }
        }
    });

    // Callback 2: Project conversation clicked
    window.on_sidebar_project_conversation_clicked({
        let window_weak = window.as_weak();
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

                load_chat_session(&win, &session_id, project_path.as_deref(), &app_state);
            }
        }
    });

    // Callback 3: "+" Clicked on sidebar to start a new general session
    window.on_sidebar_new_chat_clicked({
        let window_weak = window.as_weak();
        let app_state = Rc::clone(&state);
        move || {
            if let Some(win) = window_weak.upgrade() {
                println!("[operon-gui][sidebar] Creating new standalone chat.");
                {
                    let mut g_state = app_state.borrow_mut();
                    g_state.set_active_session_id(None);
                    g_state.set_current_project_dir(None);
                }
                clear_sidebar_selection(&win);
                win.set_session_title("New Chat".into());
            }
        }
    });

    // Callback 4: Start new session for a specific project folder
    window.on_sidebar_new_session_clicked({
        let window_weak = window.as_weak();
        let app_state = Rc::clone(&state);
        move |proj_path, proj_idx| {
            if let Some(win) = window_weak.upgrade() {
                println!("[operon-gui][sidebar] Creating new project chat for workspace: {}", proj_path);
                {
                    let mut g_state = app_state.borrow_mut();
                    g_state.set_active_session_id(None);
                    g_state.set_current_project_dir(Some(proj_path.to_string()));
                }
                win.set_active_project_index(proj_idx);
                win.set_active_conversation_index(-1);
                win.set_active_chat_index(-1);
                win.set_session_title("New Chat".into());
            }
        }
    });

    // Callback 5: Delete a standalone chat session
    window.on_sidebar_delete_chat_clicked({
        let window_weak = window.as_weak();
        let app_state = Rc::clone(&state);
        move |session_id, chat_idx| {
            let confirmed = MessageDialog::new()
                .set_title("Delete Chat Session")
                .set_description("Are you sure you want to delete this chat session?")
                .set_level(MessageLevel::Warning)
                .set_buttons(MessageButtons::OkCancel)
                .show();

            if confirmed == MessageDialogResult::Ok {
                // Clear state on the main thread first
                {
                    let mut g_state = app_state.borrow_mut();
                    if g_state.active_session_id() == Some(&session_id) {
                        g_state.set_active_session_id(None);
                    }
                }

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
                            if win.get_active_chat_index() == chat_idx {
                                win.set_session_title("New Chat".into());
                            }
                            clear_sidebar_selection(&win);
                            refresh_sidebar(&win);
                        }
                    });
                });
            }
        }
    });

    // Callback 6: Delete a project-bound conversation session
    window.on_sidebar_delete_conversation_clicked({
        let window_weak = window.as_weak();
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
                {
                    let mut g_state = app_state.borrow_mut();
                    if g_state.active_session_id() == Some(&session_id) {
                        g_state.set_active_session_id(None);
                    }
                }

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
                                win.set_session_title("New Chat".into());
                            }
                            clear_sidebar_selection(&win);
                            refresh_sidebar(&win);
                        }
                    });
                });
            }
        }
    });

    // Callback 7: Delete a project folder and all its associated chat sessions
    window.on_sidebar_delete_project_clicked({
        let window_weak = window.as_weak();
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
                let active_project_deleted = {
                    let mut g_state = app_state.borrow_mut();
                    if g_state.current_project_dir() == Some(&proj_path) {
                        g_state.set_active_session_id(None);
                        g_state.set_current_project_dir(None);
                        true
                    } else {
                        false
                    }
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
                                win.set_session_title("New Chat".into());
                            }
                            if win.get_active_project_index() == proj_idx {
                                clear_sidebar_selection(&win);
                            }
                            refresh_sidebar(&win);
                        }
                    });
                });
            }
        }
    });

    // Callback 8: Open Project folder picker (called from titlebar menu "Files" -> "Open project")
    window.on_open_project_requested({
        let window_weak = window.as_weak();
        let app_state = Rc::clone(&state);
        move || {
            let picked_folder = rfd::FileDialog::new()
                .pick_folder();

            if let Some(path_buf) = picked_folder {
                let path_str = path_buf.to_string_lossy().to_string();
                println!("[operon-gui][sidebar] User picked folder to open project: {}", path_str);

                // Update AppState directly on the main thread!
                {
                    let mut g_state = app_state.borrow_mut();
                    g_state.set_active_session_id(None);
                    g_state.set_current_project_dir(Some(path_str.clone()));
                }

                let win_weak = window_weak.clone();
                let path_str_clone = path_str.clone();

                tokio::spawn(async move {
                    // Add folder path to config.toml allowed list (idempotent)
                    if let Ok(_) = operon_rs::config::add_allowed_directory(&path_str_clone) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(win) = win_weak.upgrade() {
                                // Clear selections
                                clear_sidebar_selection(&win);
                                win.set_session_title("New Chat".into());
                                refresh_sidebar(&win);
                            }
                        });
                    }
                });
            }
        }
    });
}
