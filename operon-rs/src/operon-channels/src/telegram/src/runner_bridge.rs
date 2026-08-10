// runner_bridge.rs — Operon SessionRunner bridge for Telegram channel.
//
// Hey friend! This file bridges Telegram incoming messages to `operon_session::SessionRunner`.
//
// Flow per inbound turn:
//   1. Check if first-time user -> auto-send onboarding documentation over Telegram.
//   2. Provision shared workspace & generate role-specific channel instructions in-memory.
//   3. Compute JSON session store path (`~/.operon/sessions/telegram/<chat_id>/<session_id>.json`).
//   4. Construct `SessionConfig` with `Role::Owner` or `Role::External`.
//   5. Open SessionStore, load prior history if session file exists, compute turn_index / last_token_count.
//   6. Instantiate `SessionRunner` and call `set_history()` if resuming an existing session.
//   7. Execute turn and listen to `SessionEvent` stream:
//      - `ToolCallStart`: send tool progress update (e.g. `⚡ *Executing:* web_search`).
//      - `TextDelta` / `Done`: format and send response payload back over Telegram outbound queue.
//
// Turn persistence is automatic inside `SessionRunner` — `loop_impl.rs` calls
// `store.save_turn()` after each assistant message. No extra `append_turn()` call needed here.

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

use operon_config::AppConfig;
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::CallerRole;
use operon_session::store::SessionStore;
use operon_session::{SessionConfig, SessionRunner};

use crate::error::TelegramError;
use crate::outbound::{format_for_telegram, TelegramOutboundMessage};
use crate::router::TelegramRouter;
use crate::types::ChatId;
use crate::workspace::TelegramWorkspaceManager;

/// Bridge that drives `SessionRunner` for a specific Telegram chat and sends output over Telegram outbound queue.
pub struct SessionRunnerBridge {
    app_config: AppConfig,
    workspace_manager: TelegramWorkspaceManager,
    outbound_tx: mpsc::Sender<TelegramOutboundMessage>,
    router: Option<Arc<TelegramRouter>>,
}

impl SessionRunnerBridge {
    /// Creates a new `SessionRunnerBridge` with loaded `AppConfig` and outbound message channel sender.
    pub fn new(
        app_config: AppConfig,
        workspace_manager: TelegramWorkspaceManager,
        outbound_tx: mpsc::Sender<TelegramOutboundMessage>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: None,
        }
    }

    /// Creates a new `SessionRunnerBridge` wired with `TelegramRouter` to support turn cancellation on `/new`.
    pub fn with_router(
        app_config: AppConfig,
        workspace_manager: TelegramWorkspaceManager,
        outbound_tx: mpsc::Sender<TelegramOutboundMessage>,
        router: Arc<TelegramRouter>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: Some(router),
        }
    }

    /// Auto-sends first-time onboarding documentation message over Telegram.
    pub async fn send_onboarding(&self, chat: &ChatId) -> Result<(), TelegramError> {
        let text = format!(
            "👋 *Welcome to Operon!*\n\n\
             I am your autonomous AI assistant running locally on Operon.\n\n\
             💡 *Shortcuts & Tips:*\n\
             • Send `/new` anytime to start a fresh, clean session.\n\
             • You can ask questions, run web searches, analyze files, and manage tasks.\n\n\
             _Starting your session now..._"
        );
        let msg = TelegramOutboundMessage::new(chat.as_i64(), &text);
        let _ = self.outbound_tx.send(msg).await;
        Ok(())
    }

    /// Process a turn for a chat message over Telegram.
    pub async fn process_turn(
        &self,
        chat: &ChatId,
        session_id: &str,
        role: CallerRole,
        user_message: String,
        is_first_time: bool,
    ) -> Result<(), TelegramError> {
        // Send onboarding doc on first message from this chat
        if is_first_time {
            let _ = self.send_onboarding(chat).await;
        }

        // 1. Provision user workspace & role-specific channel instructions
        let is_owner = matches!(role, CallerRole::Owner);
        let workspace_root = self
            .workspace_manager
            .provision_workspace(chat, is_owner)?;

        let channel_instructions = if is_owner {
            crate::workspace::generate_owner_channel_instructions(chat)
        } else {
            crate::workspace::generate_external_channel_instructions(chat)
        };

        // 2. Compute JSON session store path
        let store_path = self
            .workspace_manager
            .session_file_path_for(chat, session_id);

        // Map CallerRole to context Role
        let context_role = match role {
            CallerRole::Owner => operon_context::Role::Owner,
            CallerRole::External => operon_context::Role::External,
        };

        // 3. Construct SessionConfig
        let session_config = SessionConfig {
            provider_config: self.app_config.provider.clone(),
            policy: self.app_config.policy.clone(),
            project_dir: None,
            workspace_root: workspace_root.clone(),
            role: context_role,
            tool_groups: vec!["fs".into(), "shell".into(), "web".into(), "todo".into()],
            compaction: operon_context::CompactionConfig::default(),
            store_path: Some(store_path.clone()),
            channel_instructions: Some(channel_instructions),
        };

        // ── 4. Session history loading ──────────────────────────────────────────
        let is_new_session = is_first_time && !store_path.exists();

        // Open the SessionStore at store_path (creates parent dirs if needed).
        let store = SessionStore::open(&store_path)
            .await
            .map_err(|e| TelegramError::Session(e.to_string()))?;

        // For brand new sessions, create the session record in the JSON store first.
        if is_new_session {
            store
                .create_session(
                    session_id,
                    &workspace_root.to_string_lossy(),
                    session_config.provider_config.model_id(),
                    &format!("{:?}", session_config.provider_config.provider),
                )
                .await
                .map_err(|e| TelegramError::Session(e.to_string()))?;
        }

        // Load prior turn history from the store
        let history_turns = store
            .load_turns(session_id)
            .await
            .map_err(|e| TelegramError::Session(e.to_string()))?;

        let turn_index = history_turns.len();
        let last_token_count = store
            .get_last_token_count(session_id)
            .await
            .map_err(|e| TelegramError::Session(e.to_string()))?;

        info!(
            "Telegram session {} for chat {}: is_new={}, history_turns={}, turn_index={}, last_token_count={:?}",
            session_id, chat, is_new_session, history_turns.len(), turn_index, last_token_count
        );

        // ── 5. Create mpsc channels for SessionEvent and SessionCommand ─────
        let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(100);
        let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(10);

        // Wire cmd_tx to the router so /new can cancel in-flight turns!
        if let Some(ref router) = self.router {
            router.register_cmd_tx(chat, session_id, cmd_tx).await;
        }

        // ── 6. Instantiate SessionRunner and restore history ────────────────
        let mut runner = SessionRunner::new(session_config, event_tx, cmd_rx)
            .await
            .map_err(|e| TelegramError::Session(e.to_string()))?;

        if !history_turns.is_empty() {
            let history = history_turns.last().cloned().unwrap_or_default();
            runner.set_history(history, turn_index, last_token_count);
        }

        // ── 7. Spawn runner task ────────────────────────────────────────────
        let runner_handle = tokio::spawn(async move { runner.run(user_message).await });

        // ── 8. Event consumer loop — forward tool progress & final text ─────
        forward_session_events_to_outbound(chat, &self.outbound_tx, &mut event_rx).await;

        // Unregister cmd_tx from router upon turn completion
        if let Some(ref router) = self.router {
            router.unregister_cmd_tx(chat, session_id).await;
        }

        // Wait for runner task to finish cleanly
        if let Err(e) = runner_handle.await {
            info!(
                "Runner handle ended (may have been aborted/cancelled): {}",
                e
            );
        }

        Ok(())
    }
}

async fn forward_session_events_to_outbound(
    chat: &ChatId,
    outbound_tx: &mpsc::Sender<TelegramOutboundMessage>,
    event_rx: &mut mpsc::Receiver<SessionEvent>,
) {
    let mut accumulated_text = String::new();
    let mut terminal_event_seen = false;

    while let Some(event) = event_rx.recv().await {
        match event {
            SessionEvent::ToolCallStart { name, .. } => {
                let progress_msg = format!("⚡ *Executing:* `{}`", name);
                let out = TelegramOutboundMessage::new(chat.as_i64(), &progress_msg);
                let _ = outbound_tx.send(out).await;
            }
            SessionEvent::TextDelta { text } => {
                accumulated_text.push_str(&text);
            }
            SessionEvent::Done => {
                send_final_text(chat, outbound_tx, &accumulated_text).await;
                terminal_event_seen = true;
                break;
            }
            SessionEvent::Error { message } => {
                error!("SessionRunner error for chat {}: {}", chat, message);
                let err_out = TelegramOutboundMessage::new(
                    chat.as_i64(),
                    &format!("❌ *Error:* {}", message),
                );
                let _ = outbound_tx.send(err_out).await;
                terminal_event_seen = true;
                break;
            }
            other => {
                tracing::debug!(?other, "SessionEvent variant intentionally ignored for Telegram forwarding");
            }
        }
    }

    if !terminal_event_seen {
        send_final_text(chat, outbound_tx, &accumulated_text).await;
    }
}

async fn send_final_text(
    chat: &ChatId,
    outbound_tx: &mpsc::Sender<TelegramOutboundMessage>,
    accumulated_text: &str,
) {
    let trimmed = accumulated_text.trim();
    if !trimmed.is_empty() {
        let chunks = format_for_telegram(trimmed);
        for chunk in chunks {
            let final_msg = TelegramOutboundMessage::new(chat.as_i64(), &chunk);
            let _ = outbound_tx.send(final_msg).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn forwards_final_text_as_soon_as_done_arrives() {
        let chat = ChatId::new(123456789);
        let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(8);
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<TelegramOutboundMessage>(8);
        let held_sender = event_tx.clone();

        event_tx
            .send(SessionEvent::TextDelta {
                text: "Hello ".to_string(),
            })
            .await
            .unwrap();
        event_tx
            .send(SessionEvent::TextDelta {
                text: "from Operon".to_string(),
            })
            .await
            .unwrap();
        event_tx.send(SessionEvent::Done).await.unwrap();

        tokio::time::timeout(
            Duration::from_secs(1),
            forward_session_events_to_outbound(&chat, &outbound_tx, &mut event_rx),
        )
        .await
        .expect("Done must release the Telegram event forwarder");

        let out = outbound_rx.recv().await.unwrap();
        assert_eq!(out.chat_id, 123456789);
        assert!(out.text.contains("Hello"));
        assert!(out.text.contains("from Operon"));
        assert!(outbound_rx.try_recv().is_err());
        drop(held_sender);
    }
}
