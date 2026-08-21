//! Telegram channel session queries for the Left Sidebar.

use crate::left_sidebar::types::{SidebarConversationDto, SidebarProjectDto};
use std::path::PathBuf;

/// Query Telegram chat IDs and session JSON files from disk and construct SidebarProjectDto items.
#[tauri::command]
pub async fn query_telegram_contacts() -> Result<Vec<SidebarProjectDto>, String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base_sessions = home.join(".operon").join("sessions").join("telegram");

    if !base_sessions.exists() {
        return Ok(Vec::new());
    }

    let mut contacts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base_sessions) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let chat_id = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if !chat_id.is_empty() {
                    let mut conversations = Vec::new();

                    if let Ok(session_files) = std::fs::read_dir(&path) {
                        for s_entry in session_files.flatten() {
                            let s_path = s_entry.path();
                            if s_path.extension().is_some_and(|e| e == "json") {
                                let session_id = s_path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("")
                                    .to_string();

                                if !session_id.is_empty() {
                                    let custom_title = std::fs::read_to_string(&s_path)
                                        .ok()
                                        .and_then(|c| {
                                            serde_json::from_str::<serde_json::Value>(&c).ok()
                                        })
                                        .and_then(|v| {
                                            v.get("title")
                                                .and_then(|t| t.as_str())
                                                .map(String::from)
                                        });

                                    let mut created_at = 0i64;
                                    let mut first_msg_text = None;

                                    if let Ok(store) =
                                        operon_rs::session::store::SessionStore::open(&s_path).await
                                    {
                                        if let Ok(rows) = store.list_sessions().await {
                                            if let Some(row) = rows.first() {
                                                created_at = row.created_at;
                                                first_msg_text = store
                                                    .get_first_user_message_text(&row.id)
                                                    .await
                                                    .ok()
                                                    .flatten();
                                            }
                                        }
                                    }

                                    let title = match custom_title {
                                        Some(t) if !t.trim().is_empty() => t,
                                        _ => match first_msg_text {
                                            Some(msg) if !msg.trim().is_empty() => {
                                                let trimmed =
                                                    msg.trim().lines().next().unwrap_or("").trim();
                                                let display_title = if trimmed.len() > 36 {
                                                    format!("{}...", &trimmed[..36])
                                                } else {
                                                    trimmed.to_string()
                                                };
                                                if display_title.is_empty() {
                                                    format!(
                                                        "Session {}",
                                                        &session_id[..session_id.len().min(8)]
                                                    )
                                                } else {
                                                    display_title
                                                }
                                            }
                                            _ => format!(
                                                "Session {}",
                                                &session_id[..session_id.len().min(8)]
                                            ),
                                        },
                                    };

                                    conversations.push(SidebarConversationDto {
                                        id: session_id,
                                        title,
                                        created_at,
                                    });
                                }
                            }
                        }
                    }

                    if !conversations.is_empty() {
                        conversations.sort_by_key(|b| std::cmp::Reverse(b.created_at));
                        contacts.push(SidebarProjectDto {
                            name: format!("Chat {}", chat_id),
                            workspace: path.to_string_lossy().to_string(),
                            conversations,
                        });
                    }
                }
            }
        }
    }

    Ok(contacts)
}
