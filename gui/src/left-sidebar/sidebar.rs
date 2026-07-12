//! Left Sidebar orchestrator and view model refresher.
//!
//! This module orchestrates the background-threaded listing and filtering of chat sessions
//! and delegates specific callback handling to `chats.rs` and `projects.rs` submodules.

use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::state::AppState;

struct SessionRecord {
    id: String,
    created_at: i64,
    workspace: String,
    title: String,
    is_project: bool,
}

/// Helper function to strip raw Windows UNC prefix for cleaner paths.
pub fn clean_unc_path(s: String) -> String {
    if s.starts_with(r"\\?\") {
        s[4..].to_string()
    } else {
        s
    }
}

/// Asynchronously queries session JSON files, matches them against allowed projects,
/// constructs Slint models, and updates the Operon window.
pub fn refresh_sidebar(window: &crate::OperonWindow) {
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
pub fn clear_sidebar_selection(window: &crate::OperonWindow) {
    window.set_active_chat_index(-1);
    window.set_active_project_index(-1);
    window.set_active_conversation_index(-1);
}

/// Handle selection and load messages of a chosen chat session.
pub fn load_chat_session(
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

    // Delegate callback wire setup to submodules
    super::chats::wire_chats(window, Rc::clone(&state));
    super::projects::wire_projects(window, Rc::clone(&state));
}
