//! General tool card wiring functions.
//!
//! Hey friend! This file manages all the logic for creating, updating, and formatting
//! general tool cards in the chat window. We've moved it here to keep the code organized,
//! separating tool representation from reasoning/thinking cards.
//!
//! Every function here takes or manipulates the ResponseState to dynamically update
//! how the assistant renders tool calls (running, arguments ready, results).

use crate::main_content::reasoning::ResponseState;
use crate::main_content::assistant_messages::markdown::ParsedMarkdownItem;

/// Called on ToolCallDetected — the streaming-phase event that fires first.
/// This creates the initial "running" card with the streaming call_id.
///
/// Hey friend! The key details here are:
/// - We flush any pending markdown text first so that the tool card is placed
///   chronologically after the streaming text.
/// - We create a new `ParsedMarkdownItem` of kind "tool" and mark its status as "running".
/// - We store the card's index in the `active_tool_calls` map so we can retrieve and update it later.
pub fn append_tool_detected(state: &mut ResponseState, stream_call_id: &str, name: &str) {
    // 1. Flush any text that has been accumulated so far to keep content ordered.
    state.flush_text();
    state.in_thinking = false;

    // 2. Determine index of the new block we are adding.
    let idx = state.current_blocks.len();
    
    // 3. Create the tool block item with default values.
    let mut tool_item = ParsedMarkdownItem::new_default(
        "tool".to_string(),
        String::new(),
        String::new(),
        Vec::new(),
    );
    tool_item.tool_name = name.to_string();
    tool_item.tool_call_id = stream_call_id.to_string();
    tool_item.tool_status = "running".to_string();
    tool_item.tool_title = get_tool_friendly_title(name, "", false);

    // 4. Push it to the current blocks and map it.
    state.current_blocks.push(tool_item);
    state.active_tool_calls.insert(stream_call_id.to_string(), idx);
}

/// Called on ToolCallStart — the post-parse event with the real provider
/// call_id. If a streaming card already exists for this tool (created by
/// ToolCallDetected), we re-key it. Otherwise we create a new card.
///
/// Hey friend! This is where we handle the re-keying trick:
/// - A streaming call_id has the format "turn-call" (e.g. "0-0").
/// - A final call_id contains the turn/call at the end (e.g. "toolu_xyz-0-0").
/// - We derive the streaming ID from the final ID and check if we already have
///   a "running" tool card for it. If so, we swap keys in `active_tool_calls`
///   and update the card's call_id, avoiding duplicates!
pub fn append_tool_start(state: &mut ResponseState, call_id: &str, name: &str) {
    // 1. Attempt to derive the stream ID and re-key if found.
    let stream_id = derive_stream_id(call_id);
    if let Some(stream_key) = stream_id {
        if let Some(&idx) = state.active_tool_calls.get(&stream_key) {
            // Remove the old streaming key and update with the final call_id.
            state.active_tool_calls.remove(&stream_key);
            state.active_tool_calls.insert(call_id.to_string(), idx);

            if let Some(block) = state.current_blocks.get_mut(idx) {
                block.tool_call_id = call_id.to_string();
            }
            return;
        }
    }

    // 2. Check if we already have a running card with the same tool name
    //    but no matching result yet, just in case the format did not match.
    let mut found_old_id = None;
    for (existing_id, &idx) in &state.active_tool_calls {
        if existing_id != call_id {
            if let Some(block) = state.current_blocks.get(idx) {
                if block.tool_name == name && block.tool_status == "running" {
                    found_old_id = Some((existing_id.clone(), idx));
                    break;
                }
            }
        }
    }

    if let Some((old_id, idx)) = found_old_id {
        state.active_tool_calls.remove(&old_id);
        state.active_tool_calls.insert(call_id.to_string(), idx);
        if let Some(b) = state.current_blocks.get_mut(idx) {
            b.tool_call_id = call_id.to_string();
        }
        return;
    }

    // 3. Fallback: No existing card found, so we create a fresh one.
    state.flush_text();
    state.in_thinking = false;

    let idx = state.current_blocks.len();
    let mut tool_item = ParsedMarkdownItem::new_default(
        "tool".to_string(),
        String::new(),
        String::new(),
        Vec::new(),
    );
    tool_item.tool_name = name.to_string();
    tool_item.tool_call_id = call_id.to_string();
    tool_item.tool_status = "running".to_string();
    tool_item.tool_title = get_tool_friendly_title(name, "", false);

    state.current_blocks.push(tool_item);
    state.active_tool_calls.insert(call_id.to_string(), idx);
}

/// Derives the streaming call_id from the final provider call_id.
/// The final call_id format is "{prefix}-{turn_index}-{call_index}",
/// and the streaming id was stored as "{turn_index}-{call_index}".
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

/// Handles live parameter updates as arguments parse.
///
/// Hey friend! As JSON arguments streaming completes, we parse and pretty-print them,
/// and update the friendly title of the tool card based on the arguments (e.g. file paths).
pub fn append_tool_args_ready(state: &mut ResponseState, call_id: &str, name: &str, args_json: &str) {
    if let Some(&idx) = state.active_tool_calls.get(call_id) {
        if let Some(block) = state.current_blocks.get_mut(idx) {
            // Pretty-print the JSON arguments so they read cleanly in the GUI card.
            block.tool_args = if let Ok(val) = serde_json::from_str::<serde_json::Value>(args_json) {
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| args_json.to_string())
            } else {
                args_json.to_string()
            };
            block.tool_title = get_tool_friendly_title(name, args_json, false);
        }
    }
}

/// Handles streaming tool body deltas (like file write contents).
///
/// Hey friend! Some tools stream the actual changes to their body (like file content).
/// We append those changes directly into the card's tool arguments buffer.
pub fn append_tool_body_delta(state: &mut ResponseState, call_id: &str, text: &str) {
    if let Some(&idx) = state.active_tool_calls.get(call_id) {
        if let Some(block) = state.current_blocks.get_mut(idx) {
            block.tool_args.push_str(text);
        }
    }
}

/// Handles the outcome of a tool execution. Updates the existing card
/// in-place if found, otherwise creates a completed card as fallback.
///
/// Hey friend! Here we:
/// - Mark status as "failed" if there's an error, else "completed".
/// - Update the tool result content.
/// - Recalculate the friendly title.
/// - Delegate diff overlay generation to `diff::apply_diff_overlay` for file-modifying tools.
pub fn append_tool_result(state: &mut ResponseState, call_id: &str, name: &str, is_error: bool, content_json: &str) {
    let result_text = if let Ok(val) = serde_json::from_str::<serde_json::Value>(content_json) {
        if let Some(content) = val.get("content").and_then(|c| c.as_str()) {
            content.to_string()
        } else {
            serde_json::to_string_pretty(&val).unwrap_or_else(|_| content_json.to_string())
        }
    } else {
        content_json.to_string()
    };

    if let Some(&idx) = state.active_tool_calls.get(call_id) {
        if let Some(block) = state.current_blocks.get_mut(idx) {
            block.tool_status = if is_error { "failed".to_string() } else { "completed".to_string() };
            block.tool_result = result_text;

            // Re-evaluate title using final arguments string.
            block.tool_title = get_tool_friendly_title(name, &block.tool_args, true);

            // Generate diff overlay for file-modifying tools.
            let tool_args = block.tool_args.clone();
            crate::main_content::tools::diff::apply_diff_overlay(block, name, &tool_args);
        }
    } else {
        // Fallback: create a completed card if we missed both detection events.
        let idx = state.current_blocks.len();
        let mut tool_item = ParsedMarkdownItem::new_default(
            "tool".to_string(),
            String::new(),
            String::new(),
            Vec::new(),
        );
        tool_item.tool_name = name.to_string();
        tool_item.tool_call_id = call_id.to_string();
        tool_item.tool_status = if is_error { "failed".to_string() } else { "completed".to_string() };
        tool_item.tool_result = result_text;
        tool_item.tool_title = get_tool_friendly_title(name, "", true);

        // Generate diff overlay if this is a file editor.
        let tool_args = tool_item.tool_args.clone();
        crate::main_content::tools::diff::apply_diff_overlay(&mut tool_item, name, &tool_args);

        state.current_blocks.push(tool_item);
        state.active_tool_calls.insert(call_id.to_string(), idx);
    }
}

/// Generates a human-friendly tool title matching the Tauri reference layout.
///
/// Hey friend! This translates technical tool names like `ls`, `grep_search`, `write`
/// into user-friendly descriptions (e.g. "Listing directory", "Searched directory", "Editing file").
pub fn get_tool_friendly_title(name: &str, args_json: &str, is_completed: bool) -> String {
    let val = crate::main_content::tools::diff::parse_tool_args_to_value(args_json);
    let path = val.get("path")
        .or_else(|| val.get("paths"))
        .or_else(|| val.get("dir"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
        
    let display_name = if !path.is_empty() {
        let mut path_entries = Vec::new();
        if path.contains('\n') {
            path_entries = path.split('\n').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
        } else {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                path_entries.push(trimmed);
            }
        }
        
        let file_names: Vec<String> = path_entries.into_iter().map(|p| {
            // Strip optional line ranges like :40-90 or :50-
            let clean_p = if let Some(idx) = p.rfind(':') {
                if p[idx+1..].chars().all(|c| c.is_ascii_digit() || c == '-') {
                    &p[..idx]
                } else {
                    p
                }
            } else {
                p
            };
            let parts: Vec<&str> = clean_p.split(|c| c == '/' || c == '\\').collect();
            parts.last().copied().unwrap_or(clean_p).to_string()
        }).collect();
        
        file_names.join(", ")
    } else {
        String::new()
    };

    match name {
        "write" => {
            if is_completed {
                format!("Wrote {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Writing {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "append" => {
            if is_completed {
                format!("Appended {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Appending {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "edit" => {
            if is_completed {
                format!("Edited {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Editing {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "read" => {
            if is_completed {
                format!("Read {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Reading {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "delete" => {
            if is_completed {
                format!("Deleted {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Deleting {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "ls" => {
            if is_completed {
                format!("Listed {}", if display_name.is_empty() { "directory" } else { &display_name })
            } else {
                format!("Listing {}", if display_name.is_empty() { "directory" } else { &display_name })
            }
        }
        "grep" => {
            if is_completed {
                format!("Searched {}", if display_name.is_empty() { "directory" } else { &display_name })
            } else {
                format!("Searching {}", if display_name.is_empty() { "directory" } else { &display_name })
            }
        }
        "bash" => {
            if is_completed { "Executed command".to_string() } else { "Executing command".to_string() }
        }
        "ask" => {
            if is_completed { "Asked question".to_string() } else { "Asking question".to_string() }
        }
        "web_search" => {
            if is_completed { "Searched web".to_string() } else { "Searching web".to_string() }
        }
        "web_fetch" => {
            if is_completed { "Fetched web page".to_string() } else { "Fetching web page".to_string() }
        }
        "todo_create" => {
            if is_completed { "Created TODO".to_string() } else { "Creating TODO".to_string() }
        }
        "todo_update" => {
            if is_completed { "Updated TODO".to_string() } else { "Updating TODO".to_string() }
        }
        "todo_list" => {
            if is_completed { "Listed TODOs".to_string() } else { "Listing TODOs".to_string() }
        }
        _ => {
            let mut chars = name.chars();
            let capitalized = match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            };
            if is_completed {
                format!("Ran {}", capitalized)
            } else {
                format!("Running {}", capitalized)
            }
        }
    }
}
