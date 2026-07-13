//! Send button and message submission event controller.
//!
//! Spawns background tasks to execute prompt entries using the `operon-rs` agent loop runner.

use std::cell::RefCell;
use std::rc::Rc;
use slint::ComponentHandle;

use crate::state::AppState;

/// Register message submission callback.
pub fn wire_send(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();
    let app_state = Rc::clone(&state);

    window.on_message_submitted(move |message_text| {
        println!("[operon-gui][input] Message submitted: {}", message_text);

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
        let win_weak_clone = window_weak.clone();

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
                let (_cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(100);

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
                tokio::spawn(async move {
                    if let Err(e) = runner.run(message_text.to_string()).await {
                        eprintln!("[operon-gui][send] Runner failed to process message: {}", e);
                    }
                });

                // Spawn task to read events and update context indicators in the UI
                let win_weak_event = win_weak_clone.clone();
                tokio::spawn(async move {
                    while let Some(event) = event_rx.recv().await {
                        println!("[operon-gui][send] Received session event: {:?}", event);
                        
                        match event {
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

                // Force sidebar update to list the new session and its title
                let session_id_clone = session_id.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak_clone.upgrade() {
                        crate::left_sidebar::sidebar::refresh_sidebar(&win, Some(session_id_clone));
                    }
                });

                anyhow::Ok(())
            }.await;

            if let Err(e) = run_prompt {
                eprintln!("[operon-gui][send] Failed to launch prompt run: {}", e);
            }
        });
    });
}
