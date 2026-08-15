//! Messages loading and history management Tauri commands.

use crate::main_content::work_group::{WorkGroupDto, WorkGroupItemDto};
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

            let mut text_parts = Vec::new();
            let mut work_items = Vec::new();

            for block in msg.content {
                match block {
                    operon_rs::context::ContentBlock::Text(text) => {
                        if !text.trim().is_empty() {
                            text_parts.push(text);
                        }
                    }
                    operon_rs::context::ContentBlock::Reasoning(r) => {
                        if !r.thinking.trim().is_empty() {
                            work_items.push(WorkGroupItemDto::Thinking {
                                thinking_text: r.thinking,
                                is_expanded: false,
                            });
                        }
                    }
                    operon_rs::context::ContentBlock::ToolCall(tc) => {
                        let args_str = tc.arguments.to_string();
                        let title = get_tool_friendly_title(&tc.name, &args_str);
                        work_items.push(WorkGroupItemDto::Tool {
                            call_id: tc.id.0.clone(),
                            tool_name: tc.name,
                            tool_title: title,
                            tool_args: args_str,
                            tool_result: String::new(),
                            tool_status: "completed".to_string(),
                            is_expanded: false,
                        });
                    }
                    operon_rs::context::ContentBlock::ToolResult(tr) => {
                        let result_text = match tr.content {
                            operon_rs::context::ToolContent::Text(s) => s,
                            operon_rs::context::ToolContent::Json(v) => v.to_string(),
                        };

                        // Pair with existing tool call if already present in work items
                        let mut paired = false;
                        for item in &mut work_items {
                            if let WorkGroupItemDto::Tool { call_id, tool_result, tool_status, .. } = item {
                                if *call_id == tr.call_id.0 {
                                    *tool_result = result_text.clone();
                                    *tool_status = if tr.is_error { "failed".to_string() } else { "completed".to_string() };
                                    paired = true;
                                    break;
                                }
                            }
                        }

                        if !paired {
                            let title = format!("Result: {}", tr.name);
                            work_items.push(WorkGroupItemDto::Tool {
                                call_id: tr.call_id.0,
                                tool_name: tr.name,
                                tool_title: title,
                                tool_args: String::new(),
                                tool_result: result_text,
                                tool_status: if tr.is_error { "failed".to_string() } else { "completed".to_string() },
                                is_expanded: false,
                            });
                        }
                    }
                    _ => {}
                }
            }

            let full_text = text_parts.join("\n\n");
            let work_group = if !work_items.is_empty() {
                Some(WorkGroupDto {
                    items: work_items,
                    is_active: false,
                    is_expanded: false,
                    elapsed_secs: 0,
                })
            } else {
                None
            };

            if !full_text.trim().is_empty() || work_group.is_some() {
                result.push(ChatMessageDto {
                    id: format!("{turn_idx}_{msg_idx}"),
                    role: role_str.to_string(),
                    text: full_text,
                    timestamp: timestamp_str.clone(),
                    created_at,
                    turn_index: turn_idx,
                    is_liked: false,
                    is_disliked: false,
                    work_group,
                });
            }
        }
    }

    Ok(result)
}
