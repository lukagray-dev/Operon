//! Session History database query and formatting engine.
//!
//! Hey friend! This file manages querying the session database, retrieving
//! all historic messages, formatting reasoning and tool blocks, and generating
//! a clean user/assistant/tool messages structure.

use crate::main_content::assistant_messages::markdown::ParsedMarkdownItem;

/// Loads session history from the database and returns the conversation title,
/// processed messages list, and token counts.
pub async fn load_session_history(
    session_id: &str,
) -> anyhow::Result<(
    String,
    Vec<(bool, String, Vec<ParsedMarkdownItem>)>,
    usize,
    Option<usize>,
)> {
    let paths = operon_rs::config::OperonPaths::resolve()?;
    let json_path = paths.session_db(session_id);

    if !json_path.exists() {
        anyhow::bail!("Session database not found");
    }

    let store = operon_rs::session::store::SessionStore::open(&json_path).await?;

    // Get title
    let first_msg = store
        .get_first_user_message_text(session_id)
        .await
        .ok()
        .flatten();
    let title =
        crate::main_content::title::determine_session_title(first_msg.as_deref(), "New Chat");

    // Get conversation history turns
    let mut raw_messages = Vec::new();
    if let Ok(history_turns) = store.load_turns(session_id).await {
        if let Some(last_turn) = history_turns.last() {
            let mut tool_results = std::collections::HashMap::new();
            for msg in last_turn {
                if msg.role == operon_rs::context::MessageRole::Tool {
                    for block in &msg.content {
                        if let operon_rs::context::ContentBlock::ToolResult(tr) = block {
                            tool_results.insert(tr.call_id.0.clone(), tr.clone());
                        }
                    }
                }
            }
            let mut current_assistant_items = Vec::new();
            let mut current_assistant_text_parts = Vec::new();

            for msg in last_turn {
                let is_user = msg.role == operon_rs::context::MessageRole::User;
                let is_assistant = msg.role == operon_rs::context::MessageRole::Assistant;

                if is_user {
                    if !current_assistant_items.is_empty()
                        || !current_assistant_text_parts.is_empty()
                    {
                        let text = current_assistant_text_parts.join("\n");
                        raw_messages.push((false, text, current_assistant_items.clone()));
                        current_assistant_items.clear();
                        current_assistant_text_parts.clear();
                    }

                    let mut msg_items = Vec::new();
                    let mut text_parts = Vec::new();
                    for block in &msg.content {
                        if let operon_rs::context::ContentBlock::Text(s) = block {
                            text_parts.push(s.clone());
                            let parsed = crate::main_content::assistant_messages::markdown::parse_markdown_sendable(s);
                            msg_items.extend(parsed);
                        }
                    }
                    let text = text_parts.join("\n");
                    raw_messages.push((true, text, msg_items));
                } else if is_assistant {
                    for block in &msg.content {
                        match block {
                            operon_rs::context::ContentBlock::Text(s) => {
                                current_assistant_text_parts.push(s.clone());
                                let parsed = crate::main_content::assistant_messages::markdown::parse_markdown_sendable(s);
                                current_assistant_items.extend(parsed);
                            }
                            operon_rs::context::ContentBlock::Reasoning(rb) => {
                                current_assistant_items.push(ParsedMarkdownItem::new_default(
                                    "thinking".to_string(),
                                    rb.thinking.clone(),
                                    String::new(),
                                    Vec::new(),
                                ));
                            }
                            operon_rs::context::ContentBlock::ToolCall(tc) => {
                                let call_id_str = tc.id.0.clone();
                                let mut tool_item = ParsedMarkdownItem::new_default(
                                    "tool".to_string(),
                                    String::new(),
                                    String::new(),
                                    Vec::new(),
                                );
                                tool_item.tool_name = tc.name.clone();
                                tool_item.tool_call_id = call_id_str.clone();

                                // Pretty format JSON args
                                let args_str =
                                    serde_json::to_string_pretty(&tc.arguments).unwrap_or_default();
                                tool_item.tool_args = args_str.clone();

                                if let Some(tr) = tool_results.get(&call_id_str) {
                                    tool_item.tool_status = if tr.is_error {
                                        "failed".to_string()
                                    } else {
                                        "completed".to_string()
                                    };
                                    let res_text = match &tr.content {
                                        operon_rs::context::ToolContent::Text(t) => t.clone(),
                                    };
                                    tool_item.tool_result = res_text;
                                    tool_item.tool_title =
                                        crate::main_content::tools::cards::get_tool_friendly_title(
                                            &tc.name, &args_str, true,
                                        );

                                    if tc.name == "write"
                                        || tc.name == "append"
                                        || tc.name == "edit"
                                    {
                                        tool_item.tool_is_diff = true;
                                        let (diff_lines, added, deleted) =
                                            crate::main_content::tools::diff::parse_diff(
                                                &tc.name, &args_str,
                                            );
                                        tool_item.tool_diff_lines = diff_lines;
                                        tool_item.tool_added_count = added;
                                        tool_item.tool_deleted_count = deleted;
                                    }
                                } else {
                                    tool_item.tool_status = "running".to_string();
                                    tool_item.tool_title =
                                        crate::main_content::tools::cards::get_tool_friendly_title(
                                            &tc.name, &args_str, false,
                                        );
                                }
                                current_assistant_items.push(tool_item);
                            }
                            _ => {}
                        }
                    }
                }
            }

            if !current_assistant_items.is_empty() || !current_assistant_text_parts.is_empty() {
                let text = current_assistant_text_parts.join("\n");
                raw_messages.push((false, text, current_assistant_items));
            }
        }
    }

    let last_token_count = store
        .get_last_token_count(session_id)
        .await
        .ok()
        .flatten()
        .unwrap_or(0);
    let app_config = operon_rs::load().ok();
    let context_window = app_config
        .as_ref()
        .map(|c| c.provider.model.context_window)
        .unwrap_or(128_000);

    Ok((title, raw_messages, last_token_count, Some(context_window)))
}
