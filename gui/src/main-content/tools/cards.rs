//! General tool card wiring functions.
//!
//! Hey friend! This file manages all the logic for creating, updating, and formatting
//! general tool cards in the chat window.

use crate::main_content::markdown::ParsedMarkdownItem;
use crate::main_content::reasoning::ResponseState;

/// Called on ToolCallDetected — the streaming-phase event that fires first.
pub fn append_tool_detected(state: &mut ResponseState, stream_call_id: &str, name: &str) {
    state.in_thinking = false;

    let group_idx = state.ensure_work_group_open();
    let title = get_tool_friendly_title(name, "", false);
    let mut tool_item = ParsedMarkdownItem::new_default("tool".to_string(), String::new());
    tool_item.tool_name = name.to_string();
    tool_item.tool_title = title;
    tool_item.tool_status = "running".to_string();

    if let Some(group) = state.current_blocks.get_mut(group_idx) {
        let item_idx = group.work_group_items.len();
        group.work_group_items.push(tool_item);
        state
            .active_tool_calls
            .insert(stream_call_id.to_string(), (group_idx, item_idx));
    }
}

/// Called on ToolCallStart — the post-parse event with the real provider call_id.
pub fn append_tool_start(state: &mut ResponseState, call_id: &str, name: &str) {
    let stream_id = derive_stream_id(call_id);
    if let Some(stream_key) = stream_id {
        if let Some(&(group_idx, item_idx)) = state.active_tool_calls.get(&stream_key) {
            if let Some(tool_item) = get_tool_item_mut(state, group_idx, item_idx) {
                tool_item.tool_call_id = call_id.to_string();
                tool_item.tool_name = name.to_string();
                tool_item.tool_title = get_tool_friendly_title(name, "", false);
            }
            state.active_tool_calls.remove(&stream_key);
            state
                .active_tool_calls
                .insert(call_id.to_string(), (group_idx, item_idx));
            return;
        }
    }

    state.in_thinking = false;

    let group_idx = state.ensure_work_group_open();
    let title = get_tool_friendly_title(name, "", false);
    let mut tool_item = ParsedMarkdownItem::new_default("tool".to_string(), String::new());
    tool_item.tool_name = name.to_string();
    tool_item.tool_title = title;
    tool_item.tool_status = "running".to_string();
    tool_item.tool_call_id = call_id.to_string();

    if let Some(group) = state.current_blocks.get_mut(group_idx) {
        let item_idx = group.work_group_items.len();
        group.work_group_items.push(tool_item);
        state
            .active_tool_calls
            .insert(call_id.to_string(), (group_idx, item_idx));
    }
}

fn derive_stream_id(call_id: &str) -> Option<String> {
    let parts: Vec<&str> = call_id.split('-').collect();
    if parts.len() >= 3 {
        let turn_index = parts[parts.len() - 2];
        let call_index = parts[parts.len() - 1];
        Some(format!("{}-{}", turn_index, call_index))
    } else {
        None
    }
}

pub fn append_tool_args_ready(
    state: &mut ResponseState,
    call_id: &str,
    name: &str,
    args_json: &str,
) {
    if let Some(&(group_idx, item_idx)) = state.active_tool_calls.get(call_id) {
        if let Some(tool_item) = get_tool_item_mut(state, group_idx, item_idx) {
            let pretty_args = if let Ok(val) = serde_json::from_str::<serde_json::Value>(args_json)
            {
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| args_json.to_string())
            } else {
                args_json.to_string()
            };
            tool_item.tool_args = pretty_args;
            tool_item.tool_title = get_tool_friendly_title(name, args_json, false);
            crate::main_content::tools::diff::apply_diff_overlay(tool_item, name, args_json);
        }
    }
}

pub fn append_tool_body_delta(_state: &mut ResponseState, _call_id: &str, _text: &str) {}

pub fn append_tool_result(
    state: &mut ResponseState,
    call_id: &str,
    name: &str,
    is_error: bool,
    content_json: &str,
) {
    let result_text = if let Ok(val) = serde_json::from_str::<serde_json::Value>(content_json) {
        if let Some(content) = val.get("content").and_then(|c| c.as_str()) {
            content.to_string()
        } else {
            serde_json::to_string_pretty(&val).unwrap_or_else(|_| content_json.to_string())
        }
    } else {
        content_json.to_string()
    };

    let title = get_tool_friendly_title(name, "", true);

    if let Some(&(group_idx, item_idx)) = state.active_tool_calls.get(call_id) {
        if let Some(tool_item) = get_tool_item_mut(state, group_idx, item_idx) {
            tool_item.tool_status = if is_error { "failed" } else { "completed" }.to_string();
            tool_item.tool_result = result_text;
            tool_item.tool_title = title;

            let tool_args = tool_item.tool_args.clone();
            crate::main_content::tools::diff::apply_diff_overlay(tool_item, name, &tool_args);
        }
    } else {
        state.in_thinking = false;

        let group_idx = state.ensure_work_group_open();
        let mut tool_item = ParsedMarkdownItem::new_default("tool".to_string(), String::new());
        tool_item.tool_name = name.to_string();
        tool_item.tool_title = title;
        tool_item.tool_status = if is_error { "failed" } else { "completed" }.to_string();
        tool_item.tool_result = result_text;
        tool_item.tool_call_id = call_id.to_string();

        let tool_args = tool_item.tool_args.clone();
        crate::main_content::tools::diff::apply_diff_overlay(&mut tool_item, name, &tool_args);

        if let Some(group) = state.current_blocks.get_mut(group_idx) {
            let item_idx = group.work_group_items.len();
            group.work_group_items.push(tool_item);
            state
                .active_tool_calls
                .insert(call_id.to_string(), (group_idx, item_idx));
        }
    }
}

fn get_tool_item_mut(
    state: &mut ResponseState,
    group_idx: usize,
    item_idx: usize,
) -> Option<&mut ParsedMarkdownItem> {
    state
        .current_blocks
        .get_mut(group_idx)?
        .work_group_items
        .get_mut(item_idx)
}

pub fn get_tool_friendly_title(name: &str, args_json: &str, is_completed: bool) -> String {
    let val: serde_json::Value = serde_json::from_str(args_json).unwrap_or_default();
    let path = val
        .get("path")
        .or_else(|| val.get("paths"))
        .or_else(|| val.get("dir"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let display_name = if !path.is_empty() {
        let parts: Vec<&str> = path.split(|c| c == '/' || c == '\\').collect();
        parts.last().copied().unwrap_or(path).to_string()
    } else {
        String::new()
    };

    match name {
        "write" | "edit" | "append" => {
            if is_completed {
                format!(
                    "Edited {}",
                    if display_name.is_empty() {
                        "file"
                    } else {
                        &display_name
                    }
                )
            } else {
                format!(
                    "Editing {}",
                    if display_name.is_empty() {
                        "file"
                    } else {
                        &display_name
                    }
                )
            }
        }
        "read" => {
            if is_completed {
                format!(
                    "Read {}",
                    if display_name.is_empty() {
                        "file"
                    } else {
                        &display_name
                    }
                )
            } else {
                format!(
                    "Reading {}",
                    if display_name.is_empty() {
                        "file"
                    } else {
                        &display_name
                    }
                )
            }
        }
        "ls" | "list_dir" => {
            if is_completed {
                format!(
                    "Listed {}",
                    if display_name.is_empty() {
                        "directory"
                    } else {
                        &display_name
                    }
                )
            } else {
                format!(
                    "Listing {}",
                    if display_name.is_empty() {
                        "directory"
                    } else {
                        &display_name
                    }
                )
            }
        }
        "bash" | "run_command" => {
            if is_completed {
                "Executed command".to_string()
            } else {
                "Executing command".to_string()
            }
        }
        _ => {
            if is_completed {
                format!("Ran {}", name)
            } else {
                format!("Running {}", name)
            }
        }
    }
}
