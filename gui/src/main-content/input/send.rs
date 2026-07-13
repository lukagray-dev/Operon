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
    });
    window.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));

    // 2. Clear text input area and update responding state in Slint
    window.set_input_text("".into());
    window.set_is_responding(true);

    let window_weak = window.as_weak();

    // 3. Launch tokio prompt task in the background
    tokio::spawn(async move {
        let session_id_clone = session_id.clone();
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
            let flat_history: Vec<_> = history_turns.into_iter().flatten().collect();
            let last_token_count = store.get_last_token_count(&session_id).await?;

            let mut runner = operon_rs::session::SessionRunner::new(config, event_tx, cmd_rx).await?;
            runner.set_history(flat_history, turn_index, last_token_count);

            // Run runner in background task
            let runner_handle = tokio::spawn(async move {
                runner.run(message_text.to_string()).await
            });

            // Spawn task to read events and update context indicators in the UI
            let win_weak_event = window_weak.clone();
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    println!("[operon-gui][send] Received session event: {:?}", event);
                    
                    match event {
                        operon_rs::SessionEvent::TextDelta { text } => {
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
                                    
                                    // Append or merge text delta into assistant message
                                    let needs_new = msgs.last().map_or(true, |m| m.is_user);
                                    if needs_new {
                                        let parsed = crate::main_content::assistant_messages::markdown::parse_markdown(&text);
                                        msgs.push(crate::ChatMessage {
                                            id: "".into(),
                                            is_user: false,
                                            text: text.clone().into(),
                                            time: "".into(),
                                            markdown_items: slint::ModelRc::from(Rc::new(slint::VecModel::from(parsed))),
                                        });
                                    } else if let Some(last) = msgs.last_mut() {
                                        let mut new_text = last.text.to_string();
                                        new_text.push_str(&text);
                                        last.text = new_text.clone().into();
                                        
                                        let parsed = crate::main_content::assistant_messages::markdown::parse_markdown(&new_text);
                                        last.markdown_items = slint::ModelRc::from(Rc::new(slint::VecModel::from(parsed)));
                                    }
                                    
                                    win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
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

            // Force sidebar update and turn off responding flag
            let win_weak_sidebar = window_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = win_weak_sidebar.upgrade() {
                    win.set_is_responding(false);
                    crate::left_sidebar::sidebar::refresh_sidebar(&win, Some(session_id_clone));
                }
            });

            anyhow::Ok(())
        }.await;

        if let Err(e) = run_prompt {
            eprintln!("[operon-gui][send] Failed to launch prompt run: {}", e);
            // Reset responding state on launcher failures
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = window_weak.upgrade() {
                    win.set_is_responding(false);
                }
            });
        }
    });
}
