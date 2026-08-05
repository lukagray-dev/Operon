//! Agent session events handler.
//!
//! Hey friend! This file manages listening to agent loop events in the background
//! and translating them to GUI state updates, calling slint invoke_from_event_loop.

use crate::main_content::permission as perm;
use crate::main_content::reasoning::ResponseState;
use crate::main_content::tools::cards;
use slint::Model;
use std::rc::Rc;

/// Processes incoming events from the agent runner event channel.
/// Updates the UI models on the main thread via `slint::invoke_from_event_loop`.
pub async fn handle_session_events(
    win_weak: slint::Weak<crate::OperonWindow>,
    session_id: String,
    mut event_rx: tokio::sync::mpsc::Receiver<operon_rs::SessionEvent>,
) {
    let mut response_state = ResponseState::new();

    while let Some(event) = event_rx.recv().await {
        println!("[operon-gui][executor] Received session event: {:?}", event);

        match event {
            operon_rs::SessionEvent::TextDelta { text } => {
                response_state.append_text(&text);
                let parsed_items = response_state.build_parsed_items();
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        let model = win.get_chat_messages();
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(msg) = model.row_data(i) {
                                msgs.push(msg);
                            }
                        }

                        let elements =
                            crate::main_content::markdown::to_slint_elements(parsed_items);
                        let needs_new = msgs.last().map_or(true, |m| m.is_user);
                        if needs_new {
                            msgs.push(crate::ChatMessage {
                                id: "".into(),
                                is_user: false,
                                text: "".into(),
                                time: "".into(),
                                markdown_elements: slint::ModelRc::from(Rc::new(
                                    slint::VecModel::from(elements),
                                )),
                                reasoning_text: "".into(),
                                is_thinking: false,
                            });
                        } else if let Some(last) = msgs.last_mut() {
                            last.is_thinking = false;
                            last.markdown_elements =
                                slint::ModelRc::from(Rc::new(slint::VecModel::from(elements)));
                        }

                        win.set_chat_messages(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(msgs),
                        )));
                    }
                });
            }
            operon_rs::SessionEvent::ThinkingDelta { text } => {
                response_state.append_thinking(&text);
                let parsed_items = response_state.build_parsed_items();
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        let model = win.get_chat_messages();
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(msg) = model.row_data(i) {
                                msgs.push(msg);
                            }
                        }

                        let elements =
                            crate::main_content::markdown::to_slint_elements(parsed_items);
                        let needs_new = msgs.last().map_or(true, |m| m.is_user);
                        if needs_new {
                            msgs.push(crate::ChatMessage {
                                id: "".into(),
                                is_user: false,
                                text: "".into(),
                                time: "".into(),
                                markdown_elements: slint::ModelRc::from(Rc::new(
                                    slint::VecModel::from(elements),
                                )),
                                reasoning_text: "".into(),
                                is_thinking: true,
                            });
                        } else if let Some(last) = msgs.last_mut() {
                            last.is_thinking = true;
                            last.markdown_elements =
                                slint::ModelRc::from(Rc::new(slint::VecModel::from(elements)));
                        }

                        win.set_chat_messages(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(msgs),
                        )));
                    }
                });
            }
            operon_rs::SessionEvent::ToolCallStart { call_id, name } => {
                cards::append_tool_start(&mut response_state, &call_id, &name);
                let parsed_items = response_state.build_parsed_items();
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        let model = win.get_chat_messages();
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(msg) = model.row_data(i) {
                                msgs.push(msg);
                            }
                        }
                        let elements =
                            crate::main_content::markdown::to_slint_elements(parsed_items);
                        if let Some(last) = msgs.last_mut() {
                            last.markdown_elements =
                                slint::ModelRc::from(Rc::new(slint::VecModel::from(elements)));
                        }
                        win.set_chat_messages(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(msgs),
                        )));
                    }
                });
            }
            operon_rs::SessionEvent::ToolCallArgsReady {
                call_id,
                name,
                args_json,
            } => {
                cards::append_tool_args_ready(
                    &mut response_state,
                    &call_id,
                    &name,
                    &args_json,
                );
                let parsed_items = response_state.build_parsed_items();
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        let model = win.get_chat_messages();
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(msg) = model.row_data(i) {
                                msgs.push(msg);
                            }
                        }
                        let elements =
                            crate::main_content::markdown::to_slint_elements(parsed_items);
                        if let Some(last) = msgs.last_mut() {
                            last.markdown_elements =
                                slint::ModelRc::from(Rc::new(slint::VecModel::from(elements)));
                        }
                        win.set_chat_messages(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(msgs),
                        )));
                    }
                });
            }

            operon_rs::SessionEvent::ToolCallResult {
                call_id,
                name,
                is_error,
                content_json,
            } => {
                cards::append_tool_result(
                    &mut response_state,
                    &call_id,
                    &name,
                    is_error,
                    &content_json,
                );
                let parsed_items = response_state.build_parsed_items();
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        let model = win.get_chat_messages();
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(msg) = model.row_data(i) {
                                msgs.push(msg);
                            }
                        }
                        let elements =
                            crate::main_content::markdown::to_slint_elements(parsed_items);
                        if let Some(last) = msgs.last_mut() {
                            last.markdown_elements =
                                slint::ModelRc::from(Rc::new(slint::VecModel::from(elements)));
                        }
                        win.set_chat_messages(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(msgs),
                        )));
                    }
                });
            }
            operon_rs::SessionEvent::ApprovalRequired {
                id,
                tool,
                path,
                reason,
                args_json,
            } => {
                let path_str = path.clone().unwrap_or_default();
                perm::append_approval_required(
                    &mut response_state,
                    &id,
                    &tool,
                    &path_str,
                    &reason,
                    &args_json,
                );
                let parsed_items = response_state.build_parsed_items();
                let (display_action, display_target) =
                    get_permission_display_info(&tool, &path_str, &args_json);
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        let model = win.get_chat_messages();
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(msg) = model.row_data(i) {
                                msgs.push(msg);
                            }
                        }
                        let elements =
                            crate::main_content::markdown::to_slint_elements(parsed_items);
                        if let Some(last) = msgs.last_mut() {
                            last.markdown_elements =
                                slint::ModelRc::from(Rc::new(slint::VecModel::from(elements)));
                        }
                        win.set_chat_messages(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(msgs),
                        )));

                        win.set_pending_permission_id(id.into());
                        win.set_pending_permission_tool(tool.into());
                        win.set_pending_permission_path(path_str.into());
                        win.set_pending_permission_reason(reason.into());
                        win.set_pending_permission_action(display_action.into());
                        win.set_pending_permission_target(display_target.into());
                        win.set_has_pending_permission(true);
                    }
                });
            }
            operon_rs::SessionEvent::ApprovalGranted { id, .. } => {
                perm::append_approval_resolved(&mut response_state, &id, true);
                let parsed_items = response_state.build_parsed_items();
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        let model = win.get_chat_messages();
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(msg) = model.row_data(i) {
                                msgs.push(msg);
                            }
                        }
                        let elements =
                            crate::main_content::markdown::to_slint_elements(parsed_items);
                        if let Some(last) = msgs.last_mut() {
                            last.markdown_elements =
                                slint::ModelRc::from(Rc::new(slint::VecModel::from(elements)));
                        }
                        win.set_chat_messages(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(msgs),
                        )));
                        win.set_has_pending_permission(false);
                    }
                });
            }
            operon_rs::SessionEvent::PermissionDenied {
                tool,
                path,
                reason,
            } => {
                let path_str = path.unwrap_or_default();
                perm::append_permission_denied_event(
                    &mut response_state,
                    &tool,
                    &path_str,
                    &reason,
                );
                let parsed_items = response_state.build_parsed_items();
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        let model = win.get_chat_messages();
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(msg) = model.row_data(i) {
                                msgs.push(msg);
                            }
                        }
                        let elements =
                            crate::main_content::markdown::to_slint_elements(parsed_items);
                        if let Some(last) = msgs.last_mut() {
                            last.markdown_elements =
                                slint::ModelRc::from(Rc::new(slint::VecModel::from(elements)));
                        }
                        win.set_chat_messages(slint::ModelRc::from(Rc::new(
                            slint::VecModel::from(msgs),
                        )));
                        win.set_has_pending_permission(false);
                    }
                });
            }
            operon_rs::SessionEvent::ContextUsageUpdated {
                current_context_tokens,
                context_window,
                utilization,
                ..
            } => {
                let display_text = crate::main_content::input::context::format_tokens(
                    current_context_tokens as i32,
                    context_window as i32,
                );
                let win_weak_update = win_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_update.upgrade() {
                        win.set_context_usage(utilization);
                        win.set_tokens_used(current_context_tokens as i32);
                        win.set_tokens_total(context_window as i32);
                        win.set_context_text(display_text.into());
                    }
                });
            }
            _ => {}
        }
    }

    let final_parsed_items = response_state.finalize();
    let win_weak_final = win_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(win) = win_weak_final.upgrade() {
            let model = win.get_chat_messages();
            let count = model.row_count();
            if count > 0 {
                if let Some(last_msg) = model.row_data(count - 1) {
                    if !last_msg.is_user {
                        let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                        for i in 0..count {
                            if let Some(m) = model.row_data(i) {
                                msgs.push(m);
                            }
                        }

                        let elements =
                            crate::main_content::markdown::to_slint_elements(final_parsed_items);
                        if let Some(m) = msgs.last_mut() {
                            m.is_thinking = false;
                            m.markdown_elements = slint::ModelRc::from(std::rc::Rc::new(
                                slint::VecModel::from(elements),
                            ));
                        }
                        win.set_chat_messages(slint::ModelRc::from(std::rc::Rc::new(
                            slint::VecModel::from(msgs),
                        )));
                    }
                }
            }
            win.set_is_responding(false);
            win.set_has_pending_permission(false);
            crate::left_sidebar::refresh_sidebar(&win, Some(session_id));
        }
    });
}

fn get_permission_display_info(tool: &str, path: &str, args_json: &str) -> (String, String) {
    let filename = if !path.is_empty() {
        let parts: Vec<&str> = path.split(|c| c == '/' || c == '\\').collect();
        parts.last().copied().unwrap_or(path).to_string()
    } else {
        let val: serde_json::Value = serde_json::from_str(args_json).unwrap_or_default();
        if let Some(p) = val
            .get("path")
            .or_else(|| val.get("paths"))
            .or_else(|| val.get("dir"))
            .and_then(|v| v.as_str())
        {
            let parts: Vec<&str> = p.split(|c| c == '/' || c == '\\').collect();
            parts.last().copied().unwrap_or(p).to_string()
        } else if let Some(cmd) = val
            .get("CommandLine")
            .or_else(|| val.get("command"))
            .and_then(|v| v.as_str())
        {
            cmd.to_string()
        } else {
            String::new()
        }
    };

    let action = match tool {
        "write" | "edit" | "append" => "edit".to_string(),
        "read" => "read".to_string(),
        "delete" => "delete".to_string(),
        "ls" | "list_dir" => "list files in".to_string(),
        "grep" | "grep_search" => "search directory".to_string(),
        "bash" | "run_command" => "execute command".to_string(),
        "web_search" | "search_web" => "search the web".to_string(),
        "web_fetch" | "read_url_content" => "fetch web page".to_string(),
        _ => format!("run {}", tool),
    };

    let target = if filename.is_empty() {
        match tool {
            "load_tools" | "list_tools" => "available tools".to_string(),
            _ => String::new(),
        }
    } else {
        filename
    };

    (action, target)
}
