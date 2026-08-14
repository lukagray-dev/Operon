//! WhatsApp channel session queries for the Left Sidebar.

use crate::left_sidebar::types::ChannelContactDto;

/// Queries WhatsApp contact sessions from `~/.operon/sessions/whatsapp`.
#[tauri::command]
pub async fn query_whatsapp_contacts() -> Result<Vec<ChannelContactDto>, String> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let base_sessions = home.join(".operon").join("sessions").join("whatsapp");

    if !base_sessions.exists() {
        return Ok(Vec::new());
    }

    let mut contacts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base_sessions) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let contact_num = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if !contact_num.is_empty() {
                    let mut latest_session_id = String::new();
                    let mut latest_ts = 0i64;
                    let mut last_msg = String::new();

                    if let Ok(session_files) = std::fs::read_dir(&path) {
                        for s_entry in session_files.flatten() {
                            let s_path = s_entry.path();
                            if s_path.extension().map_or(false, |e| e == "json") {
                                if let Ok(store) = operon_rs::session::store::SessionStore::open(&s_path).await {
                                    if let Ok(rows) = store.list_sessions().await {
                                        if let Some(row) = rows.first() {
                                            if row.created_at >= latest_ts {
                                                latest_ts = row.created_at;
                                                latest_session_id = row.id.clone();
                                                if let Ok(Some(first)) = store.get_first_user_message_text(&row.id).await {
                                                    last_msg = first;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !latest_session_id.is_empty() {
                        contacts.push(ChannelContactDto {
                            id: latest_session_id,
                            name: format!("+{}", contact_num),
                            number: contact_num,
                            last_message: last_msg,
                            last_timestamp: latest_ts,
                            unread_count: 0,
                        });
                    }
                }
            }
        }
    }

    contacts.sort_by(|a, b| b.last_timestamp.cmp(&a.last_timestamp));
    Ok(contacts)
}
