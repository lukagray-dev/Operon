//! Messages loading and history management Tauri commands.

use super::types::ChatMessageDto;

/// Formats a Unix timestamp into a relative human-friendly string.
pub fn format_timestamp(created_at: i64) -> String {
    if created_at <= 0 {
        return "Just now".to_string();
    }

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let diff = now_secs.saturating_sub(created_at);

    if diff < 60 {
        "Just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{mins}m ago")
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{hours}h ago")
    } else {
        let days = diff / 86400;
        format!("{days}d ago")
    }
}

/// Loads all messages for a specific session ID in chronological order.
#[tauri::command]
pub async fn load_session_messages(session_id: String) -> Result<Vec<ChatMessageDto>, String> {
    if session_id.trim().is_empty() {
        return Ok(Vec::new());
    }

    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let db_path = paths.session_db(&session_id);

    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let store = operon_rs::session::store::SessionStore::open(&db_path)
        .await
        .map_err(|e| e.to_string())?;

    let turns = store
        .load_turns_with_timestamps(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();

    for (turn_idx, (created_at, messages)) in turns.into_iter().enumerate() {
        let timestamp_str = format_timestamp(created_at);

        for (msg_idx, msg) in messages.into_iter().enumerate() {
            let role_str = match msg.role {
                operon_rs::context::MessageRole::User => "user",
                operon_rs::context::MessageRole::Assistant => "assistant",
                operon_rs::context::MessageRole::System => "system",
                operon_rs::context::MessageRole::Tool => "tool",
            };

            // Extract plain text from content blocks
            let mut text_parts = Vec::new();
            for block in msg.content {
                if let operon_rs::context::ContentBlock::Text(text) = block {
                    if !text.trim().is_empty() {
                        text_parts.push(text);
                    }
                }
            }

            let full_text = text_parts.join("\n\n");
            if !full_text.trim().is_empty() {
                result.push(ChatMessageDto {
                    id: format!("{turn_idx}_{msg_idx}"),
                    role: role_str.to_string(),
                    text: full_text,
                    timestamp: timestamp_str.clone(),
                    created_at,
                    turn_index: turn_idx,
                    is_liked: false,
                    is_disliked: false,
                });
            }
        }
    }

    Ok(result)
}
