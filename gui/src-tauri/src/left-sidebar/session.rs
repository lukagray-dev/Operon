//! Session and Project query/manipulation commands for the Left Sidebar.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::State;

use crate::left_sidebar::types::{SidebarConversationDto, SidebarDataDto, SidebarProjectDto};
use crate::shared::AppState;

/// Strips Windows UNC prefix (`\\?\`) for clean path representation.
pub fn clean_unc_path(s: String) -> String {
    if s.starts_with(r"\\?\") {
        s[4..].to_string()
    } else {
        s
    }
}

/// Generates a friendly display title from a first user message or defaults.
pub fn determine_session_title(first_msg: Option<&str>, default: &str) -> String {
    if let Some(msg) = first_msg {
        let clean = msg.trim();
        if !clean.is_empty() {
            let first_line = clean.lines().next().unwrap_or(clean);
            if first_line.chars().count() > 40 {
                let truncated: String = first_line.chars().take(40).collect();
                return format!("{}...", truncated);
            }
            return first_line.to_string();
        }
    }
    default.to_string()
}

/// Queries all session JSON files and groups them into standalone chats vs project conversations.
#[tauri::command]
pub async fn query_sidebar_data(
    search_query: String,
    state: State<'_, AppState>,
) -> Result<SidebarDataDto, String> {
    let query_lower = search_query.trim().to_lowercase();

    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
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

    struct SessionRecord {
        id: String,
        created_at: i64,
        workspace: String,
        title: String,
        is_project: bool,
    }

    let mut sessions = Vec::new();
    if sessions_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    if let Ok(store) = operon_rs::session::store::SessionStore::open(&path).await {
                        if let Ok(rows) = store.list_sessions().await {
                            if let Some(row) = rows.first() {
                                let custom_title = std::fs::read_to_string(&path)
                                    .ok()
                                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                                    .and_then(|v| v.get("title").and_then(|t| t.as_str()).map(String::from));

                                let title = match custom_title {
                                    Some(t) => t,
                                    None => {
                                        let first_msg = store
                                            .get_first_user_message_text(&row.id)
                                            .await
                                            .ok()
                                            .flatten();
                                        determine_session_title(first_msg.as_deref(), "Untitled Chat")
                                    }
                                };

                                let session_workspace_canon = {
                                    let p = PathBuf::from(&row.workspace)
                                        .canonicalize()
                                        .unwrap_or_else(|_| PathBuf::from(&row.workspace))
                                        .to_string_lossy()
                                        .to_string();
                                    clean_unc_path(p)
                                };

                                let is_project = session_workspace_canon != default_workspace;
                                let project_name = if is_project {
                                    Path::new(&row.workspace)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("")
                                        .to_string()
                                } else {
                                    String::new()
                                };

                                let matches = query_lower.is_empty()
                                    || title.to_lowercase().contains(&query_lower)
                                    || project_name.to_lowercase().contains(&query_lower);

                                if matches {
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
        }
    }

    // Sort newest first
    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let mut standalone_chats = Vec::new();
    let mut project_chats_map: HashMap<String, Vec<SidebarConversationDto>> = HashMap::new();

    for p in &projects_list {
        project_chats_map.insert(p.clone(), Vec::new());
    }

    for s in sessions {
        let dto = SidebarConversationDto {
            id: s.id,
            title: s.title,
            created_at: s.created_at,
        };

        if !s.is_project {
            standalone_chats.push(dto);
        } else {
            let entry = project_chats_map.entry(s.workspace).or_default();
            entry.push(dto);
        }
    }

    let mut projects_data = Vec::new();
    for p in projects_list {
        let name = Path::new(&p)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&p)
            .to_string();

        let conversations = project_chats_map.remove(&p).unwrap_or_default();

        let project_matches = query_lower.is_empty()
            || name.to_lowercase().contains(&query_lower)
            || !conversations.is_empty();

        if project_matches {
            projects_data.push(SidebarProjectDto {
                name,
                workspace: p,
                conversations,
            });
        }
    }

    let active_session_id = {
        let lock = state.state_lock.lock().ok();
        lock.and_then(|s| s.active_project.clone())
    };

    Ok(SidebarDataDto {
        chats: standalone_chats,
        projects: projects_data,
        active_session_id,
    })
}

/// Deletes a single session JSON file from disk.
#[tauri::command]
pub async fn delete_session(session_id: String) -> Result<(), String> {
    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let json_path = paths.session_db(&session_id);
    if json_path.exists() {
        std::fs::remove_file(json_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Deletes a project folder from allowed list and removes all associated session records.
#[tauri::command]
pub async fn delete_project(project_path: String) -> Result<(), String> {
    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let sessions_dir = &paths.sessions_dir;

    let clean_proj = clean_unc_path(project_path.clone());
    let mut session_ids_to_delete = Vec::new();

    if sessions_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    if let Ok(store) = operon_rs::session::store::SessionStore::open(&path).await {
                        if let Ok(rows) = store.list_sessions().await {
                            if let Some(row) = rows.first() {
                                let clean_ws = clean_unc_path(row.workspace.clone());
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

    for id in &session_ids_to_delete {
        let json_path = paths.session_db(id);
        if json_path.exists() {
            let _ = std::fs::remove_file(json_path);
        }
    }

    let _ = operon_rs::config::remove_allowed_directory(&project_path);
    Ok(())
}

/// Opens a native folder picker dialog to select and add a project folder.
#[tauri::command]
pub async fn open_project_picker() -> Result<Option<String>, String> {
    let picked = rfd::FileDialog::new().pick_folder();

    if let Some(path_buf) = picked {
        let path_str = path_buf.to_string_lossy().to_string();
        let _ = operon_rs::config::add_allowed_directory(&path_str);
        Ok(Some(path_str))
    } else {
        Ok(None)
    }
}

/// Creates or registers a new session ID and sets active state.
#[tauri::command]
pub async fn create_new_session(
    session_id: Option<String>,
    project_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let id = session_id.unwrap_or_else(|| {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("session-{}", ts)
    });

    if let Ok(mut lock) = state.state_lock.lock() {
        lock.active_project = project_path;
    }
    Ok(id)
}

/// Renames a session by updating custom title metadata in the session store.
#[tauri::command]
pub async fn rename_session(session_id: String, new_title: String) -> Result<(), String> {
    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let json_path = paths.session_db(&session_id);
    if json_path.exists() {
        let content = std::fs::read_to_string(&json_path).map_err(|e| e.to_string())?;
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(obj) = val.as_object_mut() {
                obj.insert("title".to_string(), serde_json::Value::String(new_title));
                let formatted = serde_json::to_string_pretty(&val).map_err(|e| e.to_string())?;
                std::fs::write(&json_path, formatted).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Forks a session into a brand new session with copied message history.
#[tauri::command]
pub async fn fork_session(session_id: String) -> Result<String, String> {
    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let src_path = paths.session_db(&session_id);
    if !src_path.exists() {
        return Err(format!("Source session {session_id} not found"));
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let new_id = format!("session-{}-fork", ts);
    let dest_path = paths.session_db(&new_id);

    let content = std::fs::read_to_string(&src_path).map_err(|e| e.to_string())?;
    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(obj) = val.as_object_mut() {
            obj.insert("id".to_string(), serde_json::Value::String(new_id.clone()));
            obj.insert("created_at".to_string(), serde_json::json!(ts as i64 / 1000));
            let formatted = serde_json::to_string_pretty(&val).map_err(|e| e.to_string())?;
            std::fs::write(&dest_path, formatted).map_err(|e| e.to_string())?;
            return Ok(new_id);
        }
    }

    let modified = content.replace(&session_id, &new_id);
    std::fs::write(&dest_path, modified).map_err(|e| e.to_string())?;
    Ok(new_id)
}

/// Moves a session to a target project workspace or standalone (empty string).
#[tauri::command]
pub async fn move_session(session_id: String, target_workspace: String) -> Result<(), String> {
    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let json_path = paths.session_db(&session_id);
    if json_path.exists() {
        let content = std::fs::read_to_string(&json_path).map_err(|e| e.to_string())?;
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(obj) = val.as_object_mut() {
                obj.insert("workspace".to_string(), serde_json::Value::String(target_workspace));
                let formatted = serde_json::to_string_pretty(&val).map_err(|e| e.to_string())?;
                std::fs::write(&json_path, formatted).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
