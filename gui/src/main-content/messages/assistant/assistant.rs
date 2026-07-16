//! Assistant message actions controller.
//!
//! Handles copying assistant message text to clipboard, logging feedback (likes/dislikes),
//! and truncating/regenerating conversation turns on request.

use std::cell::RefCell;
use std::rc::Rc;
use slint::{ComponentHandle, Model};

use crate::state::AppState;

pub mod markdown;

/// Wire assistant message actions.
pub fn wire_assistant_messages(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();

    // Callback 1: Copy assistant message text to clipboard
    window.on_assistant_message_copy_clicked(move |msg_idx| {
        if let Some(win) = window_weak.upgrade() {
            let model = win.get_chat_messages();
            if let Some(msg) = model.row_data(msg_idx as usize) {
                match arboard::Clipboard::new() {
                    Ok(mut clipboard) => {
                        if let Err(e) = clipboard.set_text(msg.text.to_string()) {
                            eprintln!("[operon-gui][assistant-message] Failed to write text to clipboard: {}", e);
                        } else {
                            println!("[operon-gui][assistant-message] Copied assistant message to clipboard");
                        }
                    }
                    Err(e) => {
                        eprintln!("[operon-gui][assistant-message] Failed to open clipboard: {}", e);
                    }
                }
            }
        }
    });

    // Callback 2: Like assistant message
    window.on_assistant_message_like_clicked(move |msg_idx| {
        println!("[operon-gui][assistant-message] Liked assistant message at index {}", msg_idx);
    });

    // Callback 3: Dislike assistant message
    window.on_assistant_message_dislike_clicked(move |msg_idx| {
        println!("[operon-gui][assistant-message] Disliked assistant message at index {}", msg_idx);
    });

    let window_weak = window.as_weak();
    let app_state = Rc::clone(&state);

    // Callback 4: Regenerate assistant message
    window.on_assistant_message_regenerate_clicked(move |msg_idx| {
        let win_weak = window_weak.clone();
        
        let (session_id, project_dir) = {
            let s = app_state.borrow();
            (s.active_session_id().map(String::from), s.current_project_dir().map(String::from))
        };
        
        if let Some(session_id) = session_id {
            let turn_index = (msg_idx as usize) / 2;
            println!("[operon-gui][assistant-message] Regenerating turn index {}", turn_index);
            
            tokio::spawn(async move {
                let run_regenerate = async {
                    let paths = operon_rs::config::OperonPaths::resolve()?;
                    let json_path = paths.session_db(&session_id);
                    if json_path.exists() {
                        let file_content = std::fs::read_to_string(&json_path)?;
                        let mut session: operon_rs::session::store::SessionJson = serde_json::from_str(&file_content)?;
                        
                        // Extract prompt from user message of turn turn_index
                        let mut prompt = String::new();
                        if let Some(target_turn) = session.turns.iter().find(|t| t.turn_index == turn_index) {
                            for msg in &target_turn.messages {
                                if msg.role == operon_rs::context::MessageRole::User {
                                    let mut text_parts = Vec::new();
                                    for block in &msg.content {
                                        if let operon_rs::context::ContentBlock::Text(t) = block {
                                            text_parts.push(t.clone());
                                        }
                                    }
                                    prompt = text_parts.join("\n");
                                    break;
                                }
                            }
                        }
                        
                        if prompt.is_empty() {
                            anyhow::bail!("No prompt found to regenerate");
                        }
                        
                        // Truncate to turn_index turns
                        session.turns.truncate(turn_index);
                        let json_str = serde_json::to_string_pretty(&session)?;
                        std::fs::write(&json_path, json_str)?;
                        
                        // Update UI and re-run on main loop
                        let project_dir_clone = project_dir.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(win) = win_weak.upgrade() {
                                // Truncate UI messages list up to (msg_idx - 1) which is user message index
                                let model = win.get_chat_messages();
                                let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                                for i in 0..(msg_idx as usize - 1) {
                                    if let Some(msg) = model.row_data(i) {
                                        msgs.push(msg);
                                    }
                                }
                                win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));
                                
                                // Submit the prompt again
                                crate::executor::submit_prompt(
                                    &win,
                                    prompt,
                                    session_id,
                                    false,
                                    project_dir_clone,
                                );
                            }
                        });
                    }
                    anyhow::Ok(())
                }.await;
                
                if let Err(e) = run_regenerate {
                    eprintln!("[operon-gui][assistant-message] Failed to regenerate: {}", e);
                }
            });
        }
    });

    // Callback 5: Fork conversation at assistant message
    window.on_assistant_message_fork_clicked(move |msg_idx| {
        println!("[operon-gui][assistant-message] Fork requested at message index {}", msg_idx);
        // Placeholder / not fully implemented in Tauri ref either
    });

    let window_weak = window.as_weak();
    // Callback 6: Copy code block inside markdown content to clipboard
    window.on_code_copied(move |msg_idx, item_idx| {
        if let Some(win) = window_weak.upgrade() {
            let model = win.get_chat_messages();
            if let Some(msg) = model.row_data(msg_idx as usize) {
                if let Some(item) = msg.markdown_items.row_data(item_idx as usize) {
                    match arboard::Clipboard::new() {
                        Ok(mut clipboard) => {
                            if let Err(e) = clipboard.set_text(item.text.to_string()) {
                                eprintln!("[operon-gui][code-copy] Failed to write code text to clipboard: {}", e);
                            } else {
                                println!("[operon-gui][code-copy] Copied code block text to clipboard");
                            }
                        }
                        Err(e) => {
                            eprintln!("[operon-gui][code-copy] Failed to open clipboard: {}", e);
                        }
                    }
                }
            }
        }
    });
}
