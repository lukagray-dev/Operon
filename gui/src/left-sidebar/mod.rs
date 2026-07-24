//! Left sidebar controller orchestration.
//!
//! This module registers the sidebar view components and sets up the coordination
//! logic for displaying project and standalone chat lists.
//!
//! Hey friend! The sidebar.rs file has been completely removed:
//! - Database/filesystem scan logic lives inside `executor::session::query_sidebar_data`
//!   and `executor::session::load_session_history`.
//! - Mod.rs orchestrates all event wiring submodules and handles updating the Slint UI.

pub mod chats;
pub mod conversation;
#[path = "new-chat.rs"]
pub mod new_chat;
pub mod projects;
pub mod search;
pub mod settings;

use crate::state::AppState;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

/// Setup and wire the left sidebar view actions and data models.
pub fn wire_left_sidebar(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    // 1. Trigger initial sidebar load
    let active_id = state.borrow().active_session_id().map(String::from);
    refresh_sidebar(window, active_id);

    // 2. Delegate callback wiring to each component submodule
    new_chat::wire_new_chat(window, Rc::clone(&state));
    settings::wire_settings(window, Rc::clone(&state));
    chats::wire_chats(window, Rc::clone(&state));
    projects::wire_projects(window, Rc::clone(&state));
    search::wire_search(window, Rc::clone(&state));
}

/// Reset active conversation selection indices in Slint
pub fn clear_sidebar_selection(window: &crate::OperonWindow) {
    window.set_active_chat_index(-1);
    window.set_active_project_index(-1);
    window.set_active_conversation_index(-1);
}

/// Asynchronously queries session data via `executor::session::query_sidebar_data`,
/// formats them, constructs Slint models, and updates the Operon window.
pub fn refresh_sidebar(window: &crate::OperonWindow, active_session_id: Option<String>) {
    let window_weak = window.as_weak();
    let search_query = window.get_sidebar_search_text().to_string().to_lowercase();

    tokio::spawn(async move {
        match crate::executor::session::query_sidebar_data(search_query).await {
            Ok((standalone_chats, projects_data)) => {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = window_weak.upgrade() {
                        ui.set_sidebar_chats(ModelRc::from(Rc::new(VecModel::from(
                            standalone_chats.clone(),
                        ))));

                        let slint_projects: Vec<crate::SidebarProject> = projects_data
                            .into_iter()
                            .map(|(name, workspace, convs)| crate::SidebarProject {
                                name: name.into(),
                                workspace: workspace.into(),
                                conversations: ModelRc::from(Rc::new(VecModel::from(convs))),
                            })
                            .collect();
                        ui.set_sidebar_projects(ModelRc::from(Rc::new(VecModel::from(
                            slint_projects.clone(),
                        ))));

                        // Auto-highlight active session in sidebar if it exists
                        if let Some(ref active_id) = active_session_id {
                            if let Some(idx) =
                                standalone_chats.iter().position(|c| c.id == *active_id)
                            {
                                ui.set_active_chat_index(idx as i32);
                                ui.set_active_project_index(-1);
                                ui.set_active_conversation_index(-1);
                            } else {
                                for (p_idx, project) in slint_projects.iter().enumerate() {
                                    let convs_model = &project.conversations;
                                    for c_idx in 0..convs_model.row_count() {
                                        if let Some(conv) = convs_model.row_data(c_idx) {
                                            if conv.id == *active_id {
                                                ui.set_active_project_index(p_idx as i32);
                                                ui.set_active_conversation_index(c_idx as i32);
                                                ui.set_active_chat_index(-1);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("[operon-gui][sidebar] Failed to query sidebar data: {}", e);
            }
        }
    });
}

/// Handle selection and load messages of a chosen chat session.
pub fn load_chat_session(
    window: &crate::OperonWindow,
    session_id: &str,
    project_path: Option<&str>,
    app_state: &Rc<RefCell<AppState>>,
) {
    // Update global state variables
    {
        let mut state = app_state.borrow_mut();
        state.set_active_session_id(Some(session_id.to_string()));
        state.set_current_project_dir(project_path.map(String::from));
    }

    println!(
        "[operon-gui][sidebar] Selected session: {}, project: {:?}",
        session_id, project_path
    );

    // Retrieve conversation title and update Slint title property
    let window_weak = window.as_weak();
    let window_weak_err = window.as_weak();
    window.set_active_session_id(session_id.into());
    window.set_is_loading_session(true);
    let session_id_str = session_id.to_string();
    let session_id_str_err = session_id_str.clone();

    tokio::spawn(async move {
        let run_load = async {
            let (title, raw_messages, last_token_count, context_window_opt) =
                crate::left_sidebar::conversation::load_session_history(&session_id_str).await?;

            let context_window = context_window_opt.unwrap_or(128_000);
            let utilization = if context_window > 0 {
                last_token_count as f32 / context_window as f32
            } else {
                0.0
            };
            let context_text = crate::main_content::input::context::format_tokens(
                last_token_count as i32,
                context_window as i32,
            );

            let active_session_check = session_id_str.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = window_weak.upgrade() {
                    // Skip if the user switched to a different session in the meantime
                    if ui.get_active_session_id() != active_session_check {
                        return;
                    }
                    if ui.get_is_responding() {
                        ui.set_is_loading_session(false);
                        return;
                    }
                    crate::main_content::title::set_session_title(&ui, &title);

                    // Convert Send-safe intermediates to Slint types on UI thread (cheap, no parsing)
                    let slint_messages: Vec<crate::ChatMessage> = raw_messages
                        .into_iter()
                        .map(|(is_user, text, items)| {
                            let elements = crate::main_content::markdown::to_slint_elements(items);
                            crate::ChatMessage {
                                id: "".into(),
                                is_user,
                                text: text.into(),
                                time: "".into(),
                                markdown_elements: slint::ModelRc::from(Rc::new(
                                    slint::VecModel::from(elements),
                                )),
                                reasoning_text: "".into(),
                                is_thinking: false,
                            }
                        })
                        .collect();

                    ui.set_chat_messages(slint::ModelRc::from(Rc::new(slint::VecModel::from(
                        slint_messages,
                    ))));
                    ui.set_context_usage(utilization);
                    ui.set_tokens_used(last_token_count as i32);
                    ui.set_tokens_total(context_window as i32);
                    ui.set_context_text(context_text.into());
                    ui.set_is_loading_session(false);
                }
            });
            anyhow::Ok(())
        }
        .await;

        if run_load.is_err() {
            let active_session_check = session_id_str_err.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = window_weak_err.upgrade() {
                    if ui.get_active_session_id() != active_session_check {
                        return;
                    }
                    ui.set_is_loading_session(false);
                }
            });
        }
    });
}
