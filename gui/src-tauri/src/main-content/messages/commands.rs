use crate::main_content::work_group::{WorkGroupDto, WorkGroupItemDto};
use super::types::{ChatMessageDto, MessageBlockDto};

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

/// Generates a friendly display title for a tool call.
fn get_tool_friendly_title(name: &str, args_json: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(args_json).unwrap_or(serde_json::Value::Null);

    let path_val = parsed.get("path")
        .or_else(|| parsed.get("TargetFile"))
        .or_else(|| parsed.get("DirectoryPath"))
        .or_else(|| parsed.get("AbsolutePath"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let short_path = if !path_val.is_empty() {
        std::path::Path::new(path_val)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path_val.to_string())
    } else {
        String::new()
    };

    match name {
        "read_file" | "view_file" | "read" => {
            if !short_path.is_empty() {
                format!("Reading {short_path}")
            } else {
                "Reading file".to_string()
            }
        }
        "write_to_file" | "replace_file_content" | "multi_replace_file_content" | "write" | "edit" => {
            if !short_path.is_empty() {
                format!("Editing {short_path}")
            } else {
                "Editing file".to_string()
            }
        }
        "list_dir" | "ls" => {
            if !short_path.is_empty() {
                format!("Listing directory {short_path}")
            } else {
                "Listing directory".to_string()
            }
        }
        "grep_search" | "search" => {
            if let Some(q) = parsed.get("Query").and_then(|v| v.as_str()) {
                format!("Searching \"{q}\"")
            } else {
                "Searching codebase".to_string()
            }
        }
        "run_command" | "bash" | "exec" => {
            if let Some(cmd) = parsed.get("CommandLine").and_then(|v| v.as_str()) {
                format!("Running: {cmd}")
            } else {
                "Running command".to_string()
            }
        }
        _ => format!("Running {name}"),
    }
}

/// Locates a session JSON database file across global sessions and channel subdirectories.
pub fn find_session_db_path(session_id: &str) -> Option<std::path::PathBuf> {
    let paths = operon_rs::config::OperonPaths::resolve().ok()?;
    let direct = paths.session_db(session_id);
    if direct.exists() {
        return Some(direct);
    }

    // Check WhatsApp channels sessions: ~/.operon/sessions/whatsapp/*/<session_id>.json
    let wa_base = paths.sessions_dir.join("whatsapp");
    if wa_base.exists() {
        if let Ok(entries) = std::fs::read_dir(&wa_base) {
            for entry in entries.flatten() {
                let candidate = entry.path().join(format!("{}.json", session_id));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    // Check Telegram channels sessions: ~/.operon/sessions/telegram/*/<session_id>.json
    let tg_base = paths.sessions_dir.join("telegram");
    if tg_base.exists() {
        if let Ok(entries) = std::fs::read_dir(&tg_base) {
            for entry in entries.flatten() {
                let candidate = entry.path().join(format!("{}.json", session_id));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Loads all messages for a specific session ID in chronological order.
#[tauri::command]
pub async fn load_session_messages(session_id: String) -> Result<Vec<ChatMessageDto>, String> {
    if session_id.trim().is_empty() {
        return Ok(Vec::new());
    }

    let db_path = match find_session_db_path(&session_id) {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };

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

        let mut user_text_parts = Vec::new();
        let mut blocks: Vec<MessageBlockDto> = Vec::new();
        let mut all_assistant_text_parts = Vec::new();

        for msg in messages {
            match msg.role {
                operon_rs::context::MessageRole::User => {
                    for block in msg.content {
                        if let operon_rs::context::ContentBlock::Text(text) = block {
                            if !text.trim().is_empty() {
                                user_text_parts.push(text);
                            }
                        }
                    }
                }
                operon_rs::context::MessageRole::Assistant => {
                    for block in msg.content {
                        match block {
                            operon_rs::context::ContentBlock::Reasoning(r) => {
                                if !r.thinking.trim().is_empty() {
                                    let is_last_wg = matches!(blocks.last(), Some(MessageBlockDto::WorkGroup { .. }));
                                    if !is_last_wg {
                                        if let Some(MessageBlockDto::Text { text: prev_text }) = blocks.last_mut() {
                                            *prev_text = prev_text.trim_end().to_string();
                                        }
                                        blocks.push(MessageBlockDto::WorkGroup {
                                            data: WorkGroupDto {
                                                items: Vec::new(),
                                                is_active: false,
                                                is_expanded: false,
                                                elapsed_secs: 0,
                                            },
                                        });
                                    }

                                    if let Some(MessageBlockDto::WorkGroup { data }) = blocks.last_mut() {
                                        if let Some(WorkGroupItemDto::Thinking { thinking_text, .. }) = data.items.last_mut() {
                                            thinking_text.push_str(&r.thinking);
                                        } else {
                                            data.items.push(WorkGroupItemDto::Thinking {
                                                thinking_text: r.thinking,
                                                is_expanded: false,
                                            });
                                        }
                                    }
                                }
                            }
                            operon_rs::context::ContentBlock::ToolCall(tc) => {
                                let args_str = match &tc.arguments {
                                    serde_json::Value::String(s) => s.clone(),
                                    other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
                                };
                                let title = get_tool_friendly_title(&tc.name, &args_str);

                                let is_last_wg = matches!(blocks.last(), Some(MessageBlockDto::WorkGroup { .. }));
                                if !is_last_wg {
                                    if let Some(MessageBlockDto::Text { text: prev_text }) = blocks.last_mut() {
                                        *prev_text = prev_text.trim_end().to_string();
                                    }
                                    blocks.push(MessageBlockDto::WorkGroup {
                                        data: WorkGroupDto {
                                            items: Vec::new(),
                                            is_active: false,
                                            is_expanded: false,
                                            elapsed_secs: 0,
                                        },
                                    });
                                }

                                if let Some(MessageBlockDto::WorkGroup { data }) = blocks.last_mut() {
                                    data.items.push(WorkGroupItemDto::Tool {
                                        call_id: tc.id.0.clone(),
                                        tool_name: tc.name,
                                        tool_title: title,
                                        tool_args: args_str,
                                        tool_result: String::new(),
                                        tool_status: "completed".to_string(),
                                        is_expanded: false,
                                    });
                                }
                            }
                            operon_rs::context::ContentBlock::Text(text) => {
                                let trimmed_start = text.trim_start_matches(|c| c == '\r' || c == '\n');
                                if !trimmed_start.trim().is_empty() {
                                    all_assistant_text_parts.push(trimmed_start.to_string());
                                    if let Some(MessageBlockDto::Text { text: existing }) = blocks.last_mut() {
                                        existing.push_str("\n\n");
                                        existing.push_str(trimmed_start);
                                    } else {
                                        blocks.push(MessageBlockDto::Text { text: trimmed_start.to_string() });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                operon_rs::context::MessageRole::Tool => {
                    for block in msg.content {
                        if let operon_rs::context::ContentBlock::ToolResult(tr) = block {
                            let result_text = match tr.content {
                                operon_rs::context::ToolContent::Text(s) => s,
                                operon_rs::context::ToolContent::Json(v) => {
                                    serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())
                                }
                            };

                            let mut paired = false;
                            for b in blocks.iter_mut().rev() {
                                if let MessageBlockDto::WorkGroup { data } = b {
                                    for item in data.items.iter_mut().rev() {
                                        if let WorkGroupItemDto::Tool {
                                            call_id,
                                            tool_result,
                                            tool_status,
                                            ..
                                        } = item
                                        {
                                            if *call_id == tr.call_id.0 {
                                                *tool_result = result_text.clone();
                                                *tool_status = if tr.is_error {
                                                    "failed".to_string()
                                                } else {
                                                    "completed".to_string()
                                                };
                                                paired = true;
                                                break;
                                            }
                                        }
                                    }
                                    if paired {
                                        break;
                                    }
                                }
                            }

                            if !paired {
                                let title = format!("Result: {}", tr.name);
                                let is_last_wg = matches!(blocks.last(), Some(MessageBlockDto::WorkGroup { .. }));
                                if !is_last_wg {
                                    if let Some(MessageBlockDto::Text { text: prev_text }) = blocks.last_mut() {
                                        *prev_text = prev_text.trim_end().to_string();
                                    }
                                    blocks.push(MessageBlockDto::WorkGroup {
                                        data: WorkGroupDto {
                                            items: Vec::new(),
                                            is_active: false,
                                            is_expanded: false,
                                            elapsed_secs: 0,
                                        },
                                    });
                                }
                                if let Some(MessageBlockDto::WorkGroup { data }) = blocks.last_mut() {
                                    data.items.push(WorkGroupItemDto::Tool {
                                        call_id: tr.call_id.0,
                                        tool_name: tr.name,
                                        tool_title: title,
                                        tool_args: String::new(),
                                        tool_result: result_text,
                                        tool_status: if tr.is_error {
                                            "failed".to_string()
                                        } else {
                                            "completed".to_string()
                                        },
                                        is_expanded: false,
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // 1. Emit User Message for this turn if present
        let user_text = user_text_parts.join("\n\n");
        if !user_text.trim().is_empty() {
            result.push(ChatMessageDto {
                id: format!("turn_{turn_idx}_user"),
                role: "user".to_string(),
                text: user_text,
                timestamp: timestamp_str.clone(),
                created_at,
                turn_index: turn_idx,
                is_liked: false,
                is_disliked: false,
                work_group: None,
                blocks: None,
            });
        }

        // 2. Emit Consolidated Assistant Message for this turn if present
        let assistant_text = all_assistant_text_parts.join("\n\n");
        let first_work_group = blocks.iter().find_map(|b| {
            if let MessageBlockDto::WorkGroup { data } = b {
                if !data.items.is_empty() {
                    Some(data.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });

        if !blocks.is_empty() || !assistant_text.trim().is_empty() {
            result.push(ChatMessageDto {
                id: format!("turn_{turn_idx}_assistant"),
                role: "assistant".to_string(),
                text: assistant_text,
                timestamp: timestamp_str,
                created_at,
                turn_index: turn_idx,
                is_liked: false,
                is_disliked: false,
                work_group: first_work_group,
                blocks: if !blocks.is_empty() { Some(blocks) } else { None },
            });
        }
    }

    Ok(result)
}
