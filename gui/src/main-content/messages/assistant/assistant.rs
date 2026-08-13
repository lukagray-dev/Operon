//! Assistant message actions controller.
//!
//! Handles copying assistant message text to clipboard, logging feedback (likes/dislikes),
//! and truncating/regenerating conversation turns on request.

use slint::{ComponentHandle, Model};
use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

/// Formats a Unix timestamp (in seconds) into a clean, human-readable time string for the assistant action bar.
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

/// Wire assistant message actions.
pub fn wire_assistant_messages(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
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
        println!(
            "[operon-gui][assistant-message] Liked assistant message at index {}",
            msg_idx
        );
    });

    // Callback 3: Dislike assistant message
    window.on_assistant_message_dislike_clicked(move |msg_idx| {
        println!(
            "[operon-gui][assistant-message] Disliked assistant message at index {}",
            msg_idx
        );
    });

    let window_weak = window.as_weak();
    let app_state = Rc::clone(&state);

    // Callback 4: Regenerate assistant message
    window.on_assistant_message_regenerate_clicked(move |msg_idx| {
        let win_weak = window_weak.clone();

        let (session_id, project_dir) = {
            let s = app_state.borrow();
            (
                s.active_session_id().map(String::from),
                s.current_project_dir().map(String::from),
            )
        };

        if let Some(session_id) = session_id {
            if let Some(win) = win_weak.upgrade() {
                let model = win.get_chat_messages();
                let idx = msg_idx as usize;

                let mut user_msg_count = 0;
                let mut last_user_idx = 0;
                let mut last_user_text = String::new();

                for i in 0..=idx {
                    if let Some(msg) = model.row_data(i) {
                        if msg.is_user {
                            user_msg_count += 1;
                            last_user_idx = i;
                            last_user_text = msg.text.to_string();
                        }
                    }
                }

                if user_msg_count == 0 {
                    return;
                }
                let target_turn_index = user_msg_count - 1;

                let mut msgs: Vec<crate::ChatMessage> = Vec::new();
                for i in 0..=last_user_idx {
                    if let Some(msg) = model.row_data(i) {
                        msgs.push(msg);
                    }
                }
                win.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(msgs))));

                let cmd_tx_opt = crate::executor::get_active_cmd_tx();
                if let Some(cmd_tx) = cmd_tx_opt {
                    tokio::spawn(async move {
                        let _ = cmd_tx.send(operon_rs::SessionCommand::Cancel).await;
                    });
                }

                crate::executor::resubmit_edited_prompt(
                    &win,
                    session_id,
                    last_user_text,
                    target_turn_index,
                    project_dir,
                );
            }
        }
    });

    let window_weak_fork = window.as_weak();
    let app_state_fork = Rc::clone(&state);

    // Callback 5: Fork conversation at assistant message
    window.on_assistant_message_fork_clicked(move |msg_idx| {
        let win_weak = window_weak_fork.clone();

        let (parent_id, _project_dir) = {
            let s = app_state_fork.borrow();
            (
                s.active_session_id().map(String::from),
                s.current_project_dir().map(String::from),
            )
        };

        if let Some(parent_id) = parent_id {
            if let Some(win) = win_weak.upgrade() {
                let model = win.get_chat_messages();
                let idx = msg_idx as usize;

                let mut user_msg_count = 0;
                for i in 0..=idx {
                    if let Some(msg) = model.row_data(i) {
                        if msg.is_user {
                            user_msg_count += 1;
                        }
                    }
                }

                let keep_turns_count = user_msg_count;

                tokio::spawn(async move {
                    let run_fork = async {
                        let app_config = operon_rs::load()?;

                        let new_id = format!("{:016x}", rand_u64());

                        let parent_path = app_config.paths.session_db(&parent_id);
                        let new_path = app_config.paths.session_db(&new_id);

                        if parent_path.exists() {
                            std::fs::copy(&parent_path, &new_path)?;
                        }

                        let new_store =
                            operon_rs::session::store::SessionStore::open(&new_path).await?;
                        new_store.truncate_turns(&new_id, keep_turns_count).await?;

                        let new_id_clone = new_id.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = win_weak.upgrade() {
                                ui.set_active_session_id(new_id_clone.clone().into());
                                crate::left_sidebar::refresh_sidebar(&ui, Some(new_id_clone));
                            }
                        });

                        anyhow::Ok(())
                    }
                    .await;

                    if let Err(e) = run_fork {
                        eprintln!("[operon-gui][assistant-message] Failed to fork session: {}", e);
                    }
                });
            }
        }
    });

    let window_weak_wg = window.as_weak();

    // Callback 6: Toggle work activity group expansion in Rust model
    window.on_work_group_toggled(move |msg_idx, element_idx| {
        if let Some(win) = window_weak_wg.upgrade() {
            let model = win.get_chat_messages();
            let m_idx = msg_idx as usize;
            let e_idx = element_idx as usize;
            if let Some(mut msg) = model.row_data(m_idx) {
                let elements = msg.markdown_elements.clone();
                if e_idx < elements.row_count() {
                    if let Some(mut elem) = elements.row_data(e_idx) {
                        elem.work_group_expanded = !elem.work_group_expanded;
                        elements.set_row_data(e_idx, elem);
                        msg.markdown_elements = elements;
                        model.set_row_data(m_idx, msg);
                    }
                }
            }
        }
    });

    let window_weak_wgi = window.as_weak();

    // Callback 7: Toggle work activity item expansion in Rust model
    window.on_work_group_item_toggled(move |msg_idx, element_idx, item_idx| {
        if let Some(win) = window_weak_wgi.upgrade() {
            let model = win.get_chat_messages();
            let m_idx = msg_idx as usize;
            let e_idx = element_idx as usize;
            let i_idx = item_idx as usize;
            if let Some(mut msg) = model.row_data(m_idx) {
                let elements = msg.markdown_elements.clone();
                if e_idx < elements.row_count() {
                    if let Some(mut elem) = elements.row_data(e_idx) {
                        let items = elem.work_group_items.clone();
                        if i_idx < items.row_count() {
                            if let Some(mut item) = items.row_data(i_idx) {
                                item.item_expanded = !item.item_expanded;
                                items.set_row_data(i_idx, item);
                                elem.work_group_items = items;
                                elements.set_row_data(e_idx, elem);
                                msg.markdown_elements = elements;
                                model.set_row_data(m_idx, msg);
                            }
                        }
                    }
                }
            }
        }
    });
}

fn rand_u64() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::Instant::now().hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    hasher.finish()
}
