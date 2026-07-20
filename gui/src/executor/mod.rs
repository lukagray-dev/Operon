//! Executor Module Coordinator.
//!
//! Hey friend! This directory manages the execution of the agent loop in the background:
//! - `mod.rs` acts as the module coordinator and entry point.
//! - `session.rs` manages loading configuration, checking session database state, and initializing the SessionRunner.
//! - `events.rs` handles the event dispatch loop from the agent and updates the Slint GUI.

pub mod events;
pub mod session;

use slint::{ComponentHandle, Model};
use std::rc::Rc;
use std::sync::Mutex;

/// Global thread-safe reference to the active session's command channel.
/// This allows permission approvals/denials and cancellation from other UI parts
/// without passing thread-local Rc<RefCell<AppState>> handles into background tasks.
pub static ACTIVE_CMD_TX: Mutex<Option<tokio::sync::mpsc::Sender<operon_rs::SessionCommand>>> =
    Mutex::new(None);

/// Public getter to retrieve the active session command channel for approvals/denials.
pub fn get_active_cmd_tx() -> Option<tokio::sync::mpsc::Sender<operon_rs::SessionCommand>> {
    ACTIVE_CMD_TX.lock().unwrap().clone()
}

/// Entry point to submit a prompt to the agent session runner.
///
/// Hey friend! This function initiates the prompt turn by:
/// 1. Instantly posting the user's message to the chat view.
/// 2. Setting the GUI responding/spinner state.
/// 3. Spawning a tokio task to run the agent session in the background.
pub fn submit_prompt(
    window: &crate::OperonWindow,
    message_text: String,
    session_id: String,
    is_new_session: bool,
    project_dir: Option<String>,
) {
    println!("[operon-gui][executor] Submitting prompt: {}", message_text);

    // 1. Append user message to UI instantly so the user gets immediate feedback.
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

    // 2. Clear text input area and update responding state in Slint.
    window.set_input_text("".into());
    window.set_is_responding(true);

    let window_weak = window.as_weak();

    // 3. Launch tokio prompt task in the background.
    let session_id_clone = session_id.clone();
    tokio::spawn(async move {
        let run_prompt = async {
            // Create event/command channels for communicating with the runner.
            let (event_tx, event_rx) = tokio::sync::mpsc::channel(100);
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(100);

            // Store command sender in global static reference.
            {
                *ACTIVE_CMD_TX.lock().unwrap() = Some(cmd_tx);
            }

            // Start agent session (config loader + database initialization).
            let (mut runner, _turn_index, _last_token_count) = session::start_agent_session(
                &session_id_clone,
                is_new_session,
                project_dir,
                event_tx,
                cmd_rx,
            )
            .await?;

            // Spawn runner thread.
            let runner_handle =
                tokio::spawn(async move { runner.run(message_text.to_string()).await });

            // Spawn event handler loop.
            let win_weak_event = window_weak.clone();
            let session_id_final = session_id_clone.clone();
            tokio::spawn(async move {
                events::handle_session_events(win_weak_event, session_id_final, event_rx).await;
            });

            // Wait for runner task to complete.
            if let Ok(res) = runner_handle.await {
                if let Err(e) = res {
                    eprintln!(
                        "[operon-gui][executor] Runner failed to process message: {}",
                        e
                    );
                }
            }

            // Clear active command channel sender.
            {
                *ACTIVE_CMD_TX.lock().unwrap() = None;
            }

            anyhow::Ok(())
        }
        .await;

        if let Err(e) = run_prompt {
            eprintln!("[operon-gui][executor] Failed to launch prompt run: {}", e);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = window_weak.upgrade() {
                    win.set_is_responding(false);
                    win.set_has_pending_permission(false);
                }
            });
        }
    });
}
