//! Topbar backend Commands for Bridge.

use std::path::Path;
use super::types::TopbarDataDto;

/// Retrieves the topbar metadata for the current session and workspace.
pub async fn get_topbar_info(
    session_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<TopbarDataDto, String> {
    let mut title = "New Session".to_string();

    let mut unfinished_todo_count = 0;
    let mut total_todo_count = 0;

    if let Some(ref sid) = session_id {
        if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
            let session_file = paths.sessions_dir.join(format!("{}.json", sid));
            if session_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&session_file) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(t) = val.get("title").and_then(|v| v.as_str()) {
                            if !t.trim().is_empty() {
                                title = t.to_string();
                            }
                        }

                        if let Some(todos_arr) = val.get("todos").and_then(|v| v.as_array()) {
                            total_todo_count = todos_arr.len();
                            for item in todos_arr {
                                let status = item
                                    .get("status")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("pending");
                                if status != "completed" {
                                    unfinished_todo_count += 1;
                                }
                            }
                        }
                    }
                }
            }

            if title == "New Session" {
                let db_path = paths.session_db(sid);
                if db_path.exists() {
                    if let Ok(store) = operon_rs::session::store::SessionStore::open(&db_path).await
                    {
                        if let Ok(Some(first_msg)) = store.get_first_user_message_text(sid).await {
                            let trimmed = first_msg.trim();
                            if !trimmed.is_empty() {
                                title = trimmed
                                    .lines()
                                    .next()
                                    .unwrap_or(trimmed)
                                    .chars()
                                    .take(40)
                                    .collect();
                            }
                        }
                    }
                }
            }
        }
    }

    let is_project = workspace_path
        .as_ref()
        .is_some_and(|w| !w.trim().is_empty());
    let project_name = workspace_path.as_ref().and_then(|w| {
        Path::new(w)
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from)
    });

    Ok(TopbarDataDto {
        title,
        is_project,
        project_name,
        unfinished_todo_count,
        total_todo_count,
    })
}
