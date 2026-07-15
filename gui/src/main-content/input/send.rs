//! Send button and message submission event controller.
//!
//! Spawns background tasks to execute prompt entries using the `operon-rs` agent loop runner.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;
use slint::{ComponentHandle, Model};

use crate::state::AppState;

// Global thread-safe reference to the active session's command channel.
// This allows cancellation from titlebar/input stop buttons without passing
// thread-local Rc<RefCell<AppState>> handles into background tasks.
static ACTIVE_CMD_TX: Mutex<Option<tokio::sync::mpsc::Sender<operon_rs::SessionCommand>>> = Mutex::new(None);

/// Public getter to retrieve the active session command channel for approvals/denials.
pub fn get_active_cmd_tx() -> Option<tokio::sync::mpsc::Sender<operon_rs::SessionCommand>> {
    ACTIVE_CMD_TX.lock().unwrap().clone()
}

/// Register message submission callback.
pub fn wire_send(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();
    let app_state = Rc::clone(&state);

    window.on_message_submitted(move |message_text| {
        if let Some(win) = window_weak.upgrade() {
            // Resolve workspace settings on the main thread
            let (session_id, is_new_session) = {
                let mut s = app_state.borrow_mut();
                match s.active_session_id() {
                    Some(id) => (id.to_string(), false),
                    None => {
                        let new_id = format!("{:x}", std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos());
                        s.set_active_session_id(Some(new_id.clone()));
                        (new_id, true)
                    }
                }
            };

            let project_dir = app_state.borrow().current_project_dir().map(String::from);
            submit_prompt(&win, message_text.to_string(), session_id, is_new_session, project_dir);
        }
    });

    window.on_cancel_clicked(move || {
        println!("[operon-gui][send] Stop requested by user");
        let cmd_tx_opt = ACTIVE_CMD_TX.lock().unwrap().clone();
        if let Some(cmd_tx) = cmd_tx_opt {
            tokio::spawn(async move {
                let _ = cmd_tx.send(operon_rs::SessionCommand::Cancel).await;
            });
        }
    });
}

/// Start an agentic chat turn turn by submitting the given prompt text to the runner.
pub fn submit_prompt(
    window: &crate::OperonWindow,
    message_text: String,
    session_id: String,
    is_new_session: bool,
    project_dir: Option<String>,
) {
    println!("[operon-gui][send] Submitting prompt: {}", message_text);

    // 1. Append user message to UI instantly
    let mut msgs: Vec<crate::ChatMessage> = Vec::new();
    let model = window.get_chat_messages();
    for i in 0..model.row_count() {
        if let Some(msg) = model.row_data(i) {
            msgs.push(msg);
        }
    }
    let parsed_user = crate::main_content::user_messages::markdown::parse_markdown(&message_text);
    msgs.push(crate::ChatMessage {
        id: "".into(),
        is_user: true,
        text: message_text.clone().into(),
        time: "".into(),
        markdown_items: slint::ModelRc::from(Rc::new(slint::VecModel::from(parsed_user))),
        reasoning_text: "".into(),
        is_thinking: false,
    });
    window.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));

    // 2. Clear text input area and update responding state in Slint
    window.set_input_text("".into());
    window.set_is_responding(true);

    let window_weak = window.as_weak();

    // 3. Launch tokio prompt task in the background
    tokio::spawn(async move {
        let run_prompt = async {
            let app_config = operon_rs::load()?;
            
            let workspace_root = if let Some(ref proj) = project_dir {
                std::path::PathBuf::from(proj)
            } else {
                app_config.paths.workspace_dir.clone()
            };

            let store_path = app_config.paths.session_db(&session_id);
            
            // Construct SessionConfig
            let config = operon_rs::session::SessionConfig {
                provider_config: app_config.provider.clone(),
                policy: app_config.policy.clone(),
                project_dir: project_dir.map(std::path::PathBuf::from),
                workspace_root,
                role: operon_rs::context::Role::Owner,
                tool_groups: vec!["fs".into(), "shell".into(), "web".into(), "todo".into()],
                compaction: operon_rs::context::CompactionConfig::default(),
                store_path: Some(store_path.clone()),
            };

            // Create event/command channels
            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(100);

            // Store command sender in global static reference for cancellation support
            {
                *ACTIVE_CMD_TX.lock().unwrap() = Some(cmd_tx);
            }

            let store = operon_rs::session::store::SessionStore::open(&store_path).await?;
            
            if is_new_session {
                // Create the session record first
                store.create_session(
                    &session_id,
                    &config.workspace_root.to_string_lossy(),
                    config.provider_config.model_id(),
                    &format!("{:?}", config.provider_config.provider),
                ).await?;
            }

            // Load existing conversation turns
            let history_turns = store.load_turns(&session_id).await?;
            let turn_index = history_turns.len();
            let last_token_count = store.get_last_token_count(&session_id).await?;

            let mut runner = operon_rs::session::SessionRunner::new(config, event_tx, cmd_rx).await?;
            if !history_turns.is_empty() {
                let history = history_turns.last().cloned().unwrap_or_default();
                runner.set_history(history, turn_index, last_token_count);
            }

            // Run runner in background task
            let runner_handle = tokio::spawn(async move {
                runner.run(message_text.to_string()).await
            });

            // Spawn task to read events and update context indicators in the UI
            let win_weak_event = window_weak.clone();
            let session_id_final = session_id.clone();
            tokio::spawn(async move {
                let mut response_state = crate::main_content::reasoning::ResponseState::new();

                while let Some(event) = event_rx.recv().await {
                    println!("[operon-gui][send] Received session event: {:?}", event);
                    
                    match event {
                        operon_rs::SessionEvent::TextDelta { text } => {
                            // Hey friend! We append text deltas to our current text accumulator.
                            response_state.append_text(&text);
                            let parsed_items = response_state.build_parsed_items();
                            let text_acc = response_state.current_text_accumulator.clone();
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
                                    let model = win.get_chat_messages();
                                    let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                                    for i in 0..model.row_count() {
                                        if let Some(msg) = model.row_data(i) {
                                            msgs.push(msg);
                                        }
                                    }
                                    
                                    // Convert to Slint items safely on the UI thread
                                    let slint_items = crate::main_content::assistant_messages::markdown::to_slint_items(parsed_items);
                                    let needs_new = msgs.last().map_or(true, |m| m.is_user);
                                    if needs_new {
                                        msgs.push(crate::ChatMessage {
                                            id: "".into(),
                                            is_user: false,
                                            text: text_acc.clone().into(),
                                            time: "".into(),
                                            markdown_items: slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items))),
                                            reasoning_text: "".into(),
                                            is_thinking: false,
                                        });
                                    } else if let Some(last) = msgs.last_mut() {
                                        last.is_thinking = false;
                                        last.text = text_acc.into();
                                        last.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items)));
                                    }
                                    
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                }
                            });
                        }
                        operon_rs::SessionEvent::ThinkingDelta { text } => {
                            // Hey friend! We append reasoning deltas to our current thinking card block.
                            response_state.append_thinking(&text);
                            let parsed_items = response_state.build_parsed_items();
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
                                    let model = win.get_chat_messages();
                                    let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                                    for i in 0..model.row_count() {
                                        if let Some(msg) = model.row_data(i) {
                                            msgs.push(msg);
                                        }
                                    }
                                    
                                    // Convert to Slint items safely on the UI thread
                                    let slint_items = crate::main_content::assistant_messages::markdown::to_slint_items(parsed_items);
                                    let needs_new = msgs.last().map_or(true, |m| m.is_user);
                                    if needs_new {
                                        msgs.push(crate::ChatMessage {
                                            id: "".into(),
                                            is_user: false,
                                            text: "".into(),
                                            time: "".into(),
                                            markdown_items: slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items))),
                                            reasoning_text: "".into(),
                                            is_thinking: true,
                                        });
                                    } else if let Some(last) = msgs.last_mut() {
                                        last.is_thinking = true;
                                        last.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items)));
                                    }
                                    
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                }
                            });
                        }
                        operon_rs::SessionEvent::ToolCallStart { call_id, name } => {
                            response_state.append_tool_start(&call_id, &name);
                            let parsed_items = response_state.build_parsed_items();
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
                                    let model = win.get_chat_messages();
                                    let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                                    for i in 0..model.row_count() {
                                        if let Some(msg) = model.row_data(i) {
                                            msgs.push(msg);
                                        }
                                    }
                                    let slint_items = crate::main_content::assistant_messages::markdown::to_slint_items(parsed_items);
                                    if let Some(last) = msgs.last_mut() {
                                        last.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items)));
                                    }
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                }
                            });
                        }
                        operon_rs::SessionEvent::ToolCallArgsReady { call_id, name, args_json } => {
                            response_state.append_tool_args_ready(&call_id, &name, &args_json);
                            let parsed_items = response_state.build_parsed_items();
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
                                    let model = win.get_chat_messages();
                                    let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                                    for i in 0..model.row_count() {
                                        if let Some(msg) = model.row_data(i) {
                                            msgs.push(msg);
                                        }
                                    }
                                    let slint_items = crate::main_content::assistant_messages::markdown::to_slint_items(parsed_items);
                                    if let Some(last) = msgs.last_mut() {
                                        last.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items)));
                                    }
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                }
                            });
                        }
                        operon_rs::SessionEvent::ToolCallDetected { call_id, name, attrs: _ } => {
                            response_state.append_tool_detected(&call_id, &name);
                            let parsed_items = response_state.build_parsed_items();
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
                                    let model = win.get_chat_messages();
                                    let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                                    for i in 0..model.row_count() {
                                        if let Some(msg) = model.row_data(i) {
                                            msgs.push(msg);
                                        }
                                    }
                                    let slint_items = crate::main_content::assistant_messages::markdown::to_slint_items(parsed_items);
                                    if let Some(last) = msgs.last_mut() {
                                        last.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items)));
                                    }
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                }
                            });
                        }
                        operon_rs::SessionEvent::ToolBodyDelta { call_id, text } => {
                            response_state.append_tool_body_delta(&call_id, &text);
                            let parsed_items = response_state.build_parsed_items();
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
                                    let model = win.get_chat_messages();
                                    let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                                    for i in 0..model.row_count() {
                                        if let Some(msg) = model.row_data(i) {
                                            msgs.push(msg);
                                        }
                                    }
                                    let slint_items = crate::main_content::assistant_messages::markdown::to_slint_items(parsed_items);
                                    if let Some(last) = msgs.last_mut() {
                                        last.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items)));
                                    }
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                }
                            });
                        }
                        operon_rs::SessionEvent::ToolCallResult { call_id, name, is_error, content_json } => {
                            response_state.append_tool_result(&call_id, &name, is_error, &content_json);
                            let parsed_items = response_state.build_parsed_items();
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
                                    let model = win.get_chat_messages();
                                    let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                                    for i in 0..model.row_count() {
                                        if let Some(msg) = model.row_data(i) {
                                            msgs.push(msg);
                                        }
                                    }
                                    let slint_items = crate::main_content::assistant_messages::markdown::to_slint_items(parsed_items);
                                    if let Some(last) = msgs.last_mut() {
                                        last.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(slint_items)));
                                    }
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                }
                            });
                        }
                        operon_rs::SessionEvent::ApprovalRequired { id, tool, path, reason, args_json } => {
                            let path_str = path.clone().unwrap_or_default();
                            let (display_action, display_target) = get_permission_display_info(&tool, &path_str, &args_json);
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
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
                        operon_rs::SessionEvent::ApprovalGranted { id: _, tool: _, path: _ } => {
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
                                    win.set_has_pending_permission(false);
                                }
                            });
                        }
                        operon_rs::SessionEvent::PermissionDenied { tool: _, path: _, reason: _ } => {
                            let win_weak_update = win_weak_event.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_weak_update.upgrade() {
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
                            let win_weak_update = win_weak_event.clone();
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

                // Hey friend! Once the event channel is drained, we finalize the response block list
                // (which runs full syntect syntax highlighting on code blocks for premium aesthetics)
                // and turn off the responding spinner state.
                let final_parsed_items = response_state.finalize();
                let win_weak_final = win_weak_event.clone();
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
                                    
                                    // Convert to Slint items safely on the UI thread
                                    let final_items = crate::main_content::assistant_messages::markdown::to_slint_items(final_parsed_items);
                                    if let Some(m) = msgs.last_mut() {
                                        m.is_thinking = false;
                                        m.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(final_items)));
                                    }
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                }
                            }
                        }
                        win.set_is_responding(false);
                        win.set_has_pending_permission(false);
                        crate::left_sidebar::sidebar::refresh_sidebar(&win, Some(session_id_final));
                    }
                });
            });

            // Wait for runner task to complete
            if let Ok(res) = runner_handle.await {
                if let Err(e) = res {
                    eprintln!("[operon-gui][send] Runner failed to process message: {}", e);
                }
            }

            // Clear the active command channel sender
            {
                *ACTIVE_CMD_TX.lock().unwrap() = None;
            }

            anyhow::Ok(())
        }.await;

        if let Err(e) = run_prompt {
            eprintln!("[operon-gui][send] Failed to launch prompt run: {}", e);
            // Reset responding state on launcher failures
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = window_weak.upgrade() {
                    win.set_is_responding(false);
                    win.set_has_pending_permission(false);
                }
            });
        }
    });
}

/// Helper function to parse tool permissions into user-friendly action and target descriptions.
fn get_permission_display_info(tool: &str, path: &str, args_json: &str) -> (String, String) {
    let filename = if !path.is_empty() {
        let parts: Vec<&str> = path.split(|c| c == '/' || c == '\\').collect();
        parts.last().copied().unwrap_or(path).to_string()
    } else {
        let val: serde_json::Value = serde_json::from_str(args_json).unwrap_or_default();
        if let Some(p) = val.get("path")
            .or_else(|| val.get("paths"))
            .or_else(|| val.get("dir"))
            .and_then(|v| v.as_str())
        {
            let parts: Vec<&str> = p.split(|c| c == '/' || c == '\\').collect();
            parts.last().copied().unwrap_or(p).to_string()
        } else if let Some(cmd) = val.get("CommandLine")
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
